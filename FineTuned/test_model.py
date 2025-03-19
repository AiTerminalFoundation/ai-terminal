import os
import torch
import argparse
from transformers import AutoTokenizer, AutoModelForCausalLM
from peft import PeftModel, PeftConfig

def load_model(model_path="./llama2-7b-quantized"):
    """Load the fine-tuned model and tokenizer."""
    try:
        # Load the configuration
        config = PeftConfig.from_pretrained(model_path)
        
        # Load the base model
        base_model = AutoModelForCausalLM.from_pretrained(
            config.base_model_name_or_path,
            trust_remote_code=True,
            low_cpu_mem_usage=True,
            device_map="auto"
        )
        
        # Load the fine-tuned model
        model = PeftModel.from_pretrained(base_model, model_path)
        
        # Load tokenizer
        tokenizer = AutoTokenizer.from_pretrained(
            config.base_model_name_or_path,  # Use base model for tokenizer
            use_fast=True
        )
        
        if tokenizer.pad_token is None:
            tokenizer.pad_token = tokenizer.eos_token
            
        return model, tokenizer
    except Exception as e:
        print(f"Error loading model: {e}")
        return None, None

def generate_response(model, tokenizer, prompt, max_length=100, temperature=0.7):
    """Generate a response for the given prompt."""
    formatted_prompt = f"### Instruction:\n{prompt}\n\n### Response:"
    
    inputs = tokenizer(formatted_prompt, return_tensors="pt").to(model.device)
    
    outputs = model.generate(
        **inputs,
        max_length=max_length + inputs.input_ids.shape[1],  # Account for prompt length
        temperature=temperature,
        top_p=0.9,
        do_sample=True,
        num_return_sequences=1
    )
    
    response = tokenizer.decode(outputs[0], skip_special_tokens=True)
    
    if "### Response:" in response:
        response = response.split("### Response:")[1].strip()
    
    return response

def interactive_mode(model, tokenizer):
    """Run an interactive session with the model."""
    print("\n=== Interactive Mode ===")
    print("Type 'exit' to quit")
    
    while True:
        prompt = input("\nEnter your prompt: ")
        if prompt.lower() == 'exit':
            break
            
        response = generate_response(model, tokenizer, prompt)
        print(f"\nResponse: {response}")

def test_with_examples(model, tokenizer, examples=None):
    """Test the model with a list of example prompts."""
    if examples is None:
        examples = [
            "How do I list all files in a directory?",
            "How can I find the largest files in a directory?",
            "What's the command to check disk space?",
            "How do I search for text in files?",
            "How to compress a folder in Linux?"
        ]
    
    print("\n=== Testing with Examples ===")
    for prompt in examples:
        response = generate_response(model, tokenizer, prompt)
        print(f"\nPrompt: {prompt}")
        print(f"Response: {response}")
        print("-" * 50)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Test a fine-tuned language model")
    parser.add_argument("--model_path", type=str, default="./llama2-7b-finetuned", 
                        help="Path to the fine-tuned model (default: ./llama2-7b-finetuned)")
    parser.add_argument("--interactive", action="store_true", 
                        help="Run in interactive mode")
    parser.add_argument("--examples", action="store_true", 
                        help="Test with example prompts")
    parser.add_argument("--prompt", type=str, 
                        help="Single prompt to test")
    parser.add_argument("--temperature", type=float, default=0.7,
                        help="Temperature for generation (default: 0.7)")
    parser.add_argument("--max_length", type=int, default=100,
                        help="Maximum length for generation (default: 100)")
    
    args = parser.parse_args()
    
    device = "cuda" if torch.cuda.is_available() else "cpu"
    print(f"Using device: {device}")
    
    print(f"Loading model from {args.model_path}...")
    model, tokenizer = load_model(args.model_path)
    
    if model is None or tokenizer is None:
        print("Failed to load model. Exiting.")
        exit(1)
    
    print("Model loaded successfully")
    
    if args.prompt:
        response = generate_response(model, tokenizer, args.prompt, 
                                    args.max_length, args.temperature)
        print(f"\nPrompt: {args.prompt}")
        print(f"Response: {response}")
    
    elif args.interactive:
        interactive_mode(model, tokenizer)
    
    elif args.examples:
        test_with_examples(model, tokenizer)
    
    else:
        test_with_examples(model, tokenizer)
        interactive_mode(model, tokenizer) 