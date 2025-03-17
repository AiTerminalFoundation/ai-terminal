import os
import torch
from datasets import Dataset
from transformers import (
    AutoModelForCausalLM,
    AutoTokenizer,
    TrainingArguments,
    Trainer,
    DataCollatorForLanguageModeling,
    EarlyStoppingCallback
)
from peft import LoraConfig, get_peft_model

os.environ["HF_HUB_ENABLE_HF_TRANSFER"] = "1"
os.environ["HF_ENDPOINT"] = "https://huggingface.co"
os.makedirs("./model_cache", exist_ok=True)

input_file = "all.nl"
output_file = "all.cm"
model_id = "TinyLlama/TinyLlama-1.1B-Chat-v1.0"
output_dir = "./llama2-1.1b-finetuned"
force_cpu = False
device = "cpu" if force_cpu else ("cuda" if torch.cuda.is_available() else "cpu")


def load_data():
    """
    Loads and augments training data from input/output files.
    Returns a Dataset with original and augmented examples.
    """
    inputs = []
    outputs = []
    
    with open(input_file, 'r', encoding='utf-8') as f_in:
        inputs = [line.strip() for line in f_in]
    
    with open(output_file, 'r', encoding='utf-8') as f_out:
        outputs = [line.strip() for line in f_out]
    
    assert len(inputs) == len(outputs), "Input and output counts must match"
    
    data = []
    for i in range(len(inputs)):
        data.append({
            "input": inputs[i],
            "output": outputs[i],
            "text": f"### Instruction:\n{inputs[i]}\n\n### Response:\n{outputs[i]}"
        })
    
    augmented_data = []
    for item in data:
        augmented_data.append(item)
        
        if len(item["input"]) > 20:
            augmented_data.append({
                "input": f"Convert this to code: {item['input']}",
                "output": item["output"],
                "text": f"### Instruction:\nConvert this to code: {item['input']}\n\n### Response:\n{item['output']}"
            })
    
    return Dataset.from_list(augmented_data)


def load_model():
    """
    Loads the model and configures it for training.
    Falls back to a smaller model if the primary model fails to load.
    Returns the model and tokenizer.
    """
    tokenizer = AutoTokenizer.from_pretrained(model_id, use_fast=True)
    tokenizer.pad_token = tokenizer.eos_token
    
    load_options = {
        "cache_dir": "./model_cache",
        "low_cpu_mem_usage": True,
    }
    
    if device == "cuda" and not force_cpu:
        load_options.update({
            "torch_dtype": torch.float16,
            "device_map": "auto",
        })
    else:
        load_options.update({
            "torch_dtype": torch.float32,
            "device_map": "cpu",
        })
    
    try:
        if torch.cuda.is_available() and not force_cpu:
            torch.cuda.empty_cache()
        
        model = AutoModelForCausalLM.from_pretrained(model_id, **load_options)
        model.train()
        
        if device == "cuda" and not force_cpu:
            lora_config = LoraConfig(
                r=16,
                lora_alpha=32,
                target_modules=["q_proj", "k_proj", "v_proj", "o_proj"],
                lora_dropout=0.1,
                bias="none",
                task_type="CAUSAL_LM"
            )
            
            model = get_peft_model(model, lora_config)
            
            if hasattr(model, "active_adapters") and callable(model.active_adapters):
                original_active_adapters = model.active_adapters
                model.active_adapters = [original_active_adapters()]
            
            print("Trainable parameters:")
            model.print_trainable_parameters()
        else:
            print("Training on CPU without LoRA")
            
            for param in model.parameters():
                param.requires_grad = False
            
            for name, param in model.named_parameters():
                if "lm_head" in name or "layer_norm" in name or "layers.11" in name:
                    param.requires_grad = True
            
            trainable_params = sum(p.numel() for p in model.parameters() if p.requires_grad)
            if trainable_params == 0:
                for name, param in model.named_parameters():
                    if "layers.11" in name or "lm_head" in name:
                        param.requires_grad = True
            
            trainable_params = sum(p.numel() for p in model.parameters() if p.requires_grad)
            total_params = sum(p.numel() for p in model.parameters())
            print(f"Trainable parameters: {trainable_params} ({trainable_params/total_params:.2%} of total)")
            
            if trainable_params == 0:
                raise ValueError("No trainable parameters found. Training cannot proceed.")
        
        return model, tokenizer
    except Exception as e:
        print(f"Error loading model: {e}")
        
        fallback_model = "facebook/opt-125m"
        
        if torch.cuda.is_available() and not force_cpu:
            torch.cuda.empty_cache()
        
        model = AutoModelForCausalLM.from_pretrained(
            fallback_model,
            cache_dir="./model_cache",
            torch_dtype=torch.float32,
            low_cpu_mem_usage=True,
        )
        
        for param in model.parameters():
            param.requires_grad = False
            
        for name, param in model.named_parameters():
            if "lm_head" in name or "layer_norm" in name:
                param.requires_grad = True
        
        tokenizer = AutoTokenizer.from_pretrained(fallback_model)
        tokenizer.pad_token = tokenizer.eos_token
        
        return model, tokenizer


def prepare_model_inputs(batch):
    """
    Prepares inputs for the model by setting labels equal to input_ids.
    This is needed for causal language modeling training.
    """
    inputs = batch.copy()
    inputs["labels"] = inputs["input_ids"].clone()
    return inputs

def main():
    dataset = load_data()
    print(f"Loaded {len(dataset)} examples")
    
    if len(dataset) > 1000 and device == "cpu":
        dataset = dataset.select(range(1000))
    
    dataset = dataset.train_test_split(test_size=0.05)
    
    model, tokenizer = load_model()
    
    # Tokenize the dataset with fixed context length
    def tokenize_function(examples):
        return tokenizer(
            examples["text"], 
            padding="max_length", 
            truncation=True, 
            max_length=512  # Fixed context window size
        )
    
    tokenized_dataset = dataset.map(tokenize_function, batched=True)
    
    trainable_params = sum(p.numel() for p in model.parameters() if p.requires_grad)
    if trainable_params == 0:
        print("WARNING: No trainable parameters detected. Enabling training for some layers...")
        for name, param in model.named_parameters():
            if any(layer in name for layer in ["lm_head", "layer_norm", "layers.11", "layers.10"]):
                param.requires_grad = True
        
        trainable_params = sum(p.numel() for p in model.parameters() if p.requires_grad)
        print(f"Now trainable parameters: {trainable_params}")
        if trainable_params == 0:
            raise ValueError("Still no trainable parameters. Cannot proceed with training.")
    
    data_collator = DataCollatorForLanguageModeling(
        tokenizer=tokenizer, 
        mlm=False,
        pad_to_multiple_of=8
    )
    
    # Configure training with conservative hyperparameters
    # Small batch size with gradient accumulation to handle memory constraints
    training_args = TrainingArguments(
        output_dir=output_dir,
        per_device_train_batch_size=1,
        per_device_eval_batch_size=1,
        evaluation_strategy="steps",
        eval_steps=50,
        logging_steps=10,
        gradient_accumulation_steps=16,
        num_train_epochs=3,
        weight_decay=0.05,
        warmup_ratio=0.1,
        lr_scheduler_type="cosine",
        learning_rate=5e-5,
        save_steps=50,
        save_total_limit=3,
        fp16=False,
        gradient_checkpointing=False,
        optim="adamw_torch",
        max_grad_norm=0.3,
        ddp_find_unused_parameters=False,
        dataloader_pin_memory=False if device == "cpu" else True,
        report_to="tensorboard",
        load_best_model_at_end=True,
        metric_for_best_model="eval_loss",
        greater_is_better=False,
    )
    
    trainer = Trainer(
        model=model,
        args=training_args,
        train_dataset=tokenized_dataset["train"],
        eval_dataset=tokenized_dataset["test"],
        data_collator=data_collator,
        callbacks=[EarlyStoppingCallback(early_stopping_patience=3)]
    )
    
    model.train()
    
    # Check for existing checkpoints and resume if found
    checkpoint = None
    if os.path.exists(output_dir):
        checkpoints = [folder for folder in os.listdir(output_dir) if folder.startswith("checkpoint-")]
        if checkpoints:
            # Sort checkpoints by step number to find the latest one
            latest_checkpoint = sorted(checkpoints, key=lambda x: int(x.split("-")[1]), reverse=True)[0]
            checkpoint = os.path.join(output_dir, latest_checkpoint)
            print(f"Resuming from checkpoint: {checkpoint}")
    
    try:
        trainer.train(resume_from_checkpoint=checkpoint)
        
        if hasattr(model, "is_peft_model") and model.is_peft_model:
            try:
                if hasattr(model, "active_adapters") and isinstance(model.active_adapters, list):
                    if len(model.active_adapters) > 0 and callable(model.active_adapters[0]):
                        original_adapter = model.active_adapters[0]
                        model.active_adapters = original_adapter
                
                model.save_pretrained(output_dir)
            except Exception as peft_error:
                print(f"Error saving PEFT model: {peft_error}")
                try:
                    model.save_adapter(output_dir, "default")
                    print("Saved using adapter method instead")
                except:
                    print("Could not save using adapter method either")
        else:
            model.save_pretrained(output_dir)
        
        tokenizer.save_pretrained(output_dir)
        print(f"Model saved to {output_dir}")
    except Exception as e:
        print(f"Error during training: {e}")
        try:
            model.save_pretrained(output_dir)
            tokenizer.save_pretrained(output_dir)
            print(f"Model saved despite error to {output_dir}")
        except Exception as save_error:
            print(f"Could not save model: {save_error}")


if __name__ == "__main__":
    print(f"Using device: {device}")
    main()
