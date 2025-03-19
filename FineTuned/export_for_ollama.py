import os
import torch
import argparse
from transformers import AutoModelForCausalLM, AutoTokenizer
from peft import PeftModel, PeftConfig
import shutil
import subprocess

def merge_and_export(
    lora_model_path="./llama2-1.1b-finetuned",
    output_dir="./ollama_model",
    model_name="my-finetuned-model"
):
    """Merge LoRA weights with base model and export for Ollama."""
    print(f"Loading LoRA model from {lora_model_path}...")
    
    os.makedirs(output_dir, exist_ok=True)
    
    config = PeftConfig.from_pretrained(lora_model_path)
    base_model_name = "meta-llama/Llama-2-7b-chat-hf"
    print(f"Base model: {base_model_name}")
    
    print("Loading base model...")
    base_model = AutoModelForCausalLM.from_pretrained(
        base_model_name,
        torch_dtype=torch.float16,
        low_cpu_mem_usage=True,
        device_map="cpu"
    )
    
    print("Loading LoRA adapters...")
    model = PeftModel.from_pretrained(base_model, lora_model_path)
    
    print("Merging weights...")
    model = model.merge_and_unload()
    
    merged_model_path = os.path.join(output_dir, "merged_model")
    print(f"Saving merged model to {merged_model_path}...")
    model.save_pretrained(merged_model_path)
    
    tokenizer = AutoTokenizer.from_pretrained(base_model_name)
    tokenizer.save_pretrained(merged_model_path)
    
    model_name_for_file = base_model_name.split('/')[-1]

    modelfile_content = "FROM " + model_name_for_file + "\n"
    modelfile_content += "PARAMETER temperature 0.7\n"
    modelfile_content += "PARAMETER top_p 0.9\n"
    modelfile_content += "PARAMETER stop \"### Instruction:\"\n"
    modelfile_content += "PARAMETER stop \"### Response:\"\n\n"
    modelfile_content += "TEMPLATE \"\"\"\n"
    modelfile_content += "### Instruction:\n"
    modelfile_content += "{{.Input}}\n\n"
    modelfile_content += "### Response:\n"
    modelfile_content += "\"\"\""
    
    modelfile_path = os.path.join(output_dir, "Modelfile")
    with open(modelfile_path, "w") as f:
        f.write(modelfile_content)
    
    print(f"Created Modelfile at {modelfile_path}")
    print("\nTo create the Ollama model, run:")
    print(f"ollama create {model_name} -f {modelfile_path}")
    print(f"\nThen convert the model to GGUF format using export_for_ollama_gguf.py")
    print(f"python export_for_ollama_gguf.py --model_dir {merged_model_path} --output_dir {output_dir}/gguf")
    print(f"\nFinally, import the GGUF model into Ollama:")
    print(f"ollama import {output_dir}/gguf/{model_name}.gguf")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Export fine-tuned model for Ollama")
    parser.add_argument("--lora_model_path", type=str, default="./llama2-1.1b-finetuned2",
                        help="Path to the LoRA model")
    parser.add_argument("--output_dir", type=str, default="./ollama_model2",
                        help="Output directory for the exported model")
    parser.add_argument("--model_name", type=str, default="my-finetuned-model",
                        help="Name for the Ollama model")
    
    args = parser.parse_args()
    
    merge_and_export(args.lora_model_path, args.output_dir, args.model_name) 