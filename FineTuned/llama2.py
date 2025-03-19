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
model_id = "meta-llama/Llama-2-7b-chat-hf"
output_dir = "./llama2-1.1b-finetuned2"
force_cpu = True
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
    Loads the model and configures it for training with LoRA on both CPU and GPU.
    Adjusts LoRA parameters based on the device to optimize resource usage.
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
            "device_map": {"": 0},
        })
    else:
        load_options.update({
            "torch_dtype": torch.float32,
            "device_map": {"": "cpu"},
            "load_in_8bit": True,
        })
    
    try:
        if torch.cuda.is_available() and not force_cpu:
            torch.cuda.empty_cache()
        
        model = AutoModelForCausalLM.from_pretrained(model_id, **load_options)
        model.train()
        
        # Define LoRA configuration with adjustments for CPU
        lora_r = 4 if device == "cpu" else 16
        lora_alpha = 16 if device == "cpu" else 32
        target_modules = ["q_proj", "v_proj"] if device == "cpu" else ["q_proj", "k_proj", "v_proj", "o_proj"]
        lora_dropout = 0.05 if device == "cpu" else 0.1
        
        lora_config = LoraConfig(
            r=lora_r,
            lora_alpha=lora_alpha,
            target_modules=target_modules,
            lora_dropout=lora_dropout,
            bias="none",
            task_type="CAUSAL_LM"
        )
        
        model = get_peft_model(model, lora_config)
        
        if device == "cuda" and not force_cpu:
            print("Trainable parameters on GPU:")
        else:
            print("Training on CPU with LoRA, trainable parameters:")
        model.print_trainable_parameters()
        
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
        
        # Apply LoRA to fallback model as well
        lora_config = LoraConfig(
            r=4,
            lora_alpha=16,
            target_modules=["q_proj", "v_proj"],
            lora_dropout=0.05,
            bias="none",
            task_type="CAUSAL_LM"
        )
        model = get_peft_model(model, lora_config)
        
        tokenizer = AutoTokenizer.from_pretrained(fallback_model)
        tokenizer.pad_token = tokenizer.eos_token
        
        print("Trainable parameters in fallback model:")
        model.print_trainable_parameters()
        
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
    
    # Reduce dataset size more aggressively on CPU
    if device == "cpu":
        max_examples = 500  # Reduced from 1000
        if len(dataset) > max_examples:
            dataset = dataset.select(range(max_examples))
            print(f"Reduced dataset to {max_examples} examples for CPU training")
    
    dataset = dataset.train_test_split(test_size=0.05)
    
    model, tokenizer = load_model()
    
    # Tokenize with smaller context window on CPU
    def tokenize_function(examples):
        max_length = 256 if device == "cpu" else 512  # Smaller context window for CPU
        return tokenizer(
            examples["text"], 
            padding="max_length", 
            truncation=True, 
            max_length=max_length
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
    
    # Adjust training hyperparameters for CPU
    training_args = TrainingArguments(
        output_dir=output_dir,
        per_device_train_batch_size=1,
        per_device_eval_batch_size=1,
        evaluation_strategy="steps",
        eval_steps=50 if device == "cuda" else 20,  # Evaluate more frequently on CPU
        logging_steps=10,
        gradient_accumulation_steps=16 if device == "cuda" else 4,  # Less accumulation on CPU
        num_train_epochs=3 if device == "cuda" else 1,  # Fewer epochs on CPU
        weight_decay=0.05,
        warmup_ratio=0.1,
        lr_scheduler_type="cosine",
        learning_rate=5e-5 if device == "cuda" else 1e-4,  # Higher learning rate for fewer epochs
        save_steps=50 if device == "cuda" else 100,  # Save less frequently on CPU
        save_total_limit=3 if device == "cuda" else 1,  # Keep fewer checkpoints on CPU
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
        no_cuda=device == "cpu",
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
    
    checkpoint = None
    if os.path.exists(output_dir):
        checkpoints = [folder for folder in os.listdir(output_dir) if folder.startswith("checkpoint-")]
        if checkpoints:
            latest_checkpoint = max(checkpoints, key=lambda x: int(x.split("-")[1]))
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