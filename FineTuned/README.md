# Fine-Tuning Guide for TinyLlama with nl2bash

This guide explains how to fine-tune the **TinyLlama/TinyLlama-1.1B-Chat-v1.0** model using the **nl2bash** dataset. The dataset includes over 20,000 examples split into two files—one containing bash command descriptions and the other containing the corresponding commands.

## Prerequisites

- **Python 3.8+** installed.
- Clone this repository and the [llama2.cpp](https://github.com/ggerganov/llama.cpp) repository.
- Install required dependencies (e.g., PyTorch, Transformers).
- Ensure your environment is set up for Hugging Face LoRa format.

## Fine-Tuning Steps

1. **Run Fine-Tuning Script**  
   Execute `llama2.py` to fine-tune the model using the nl2bash dataset. This script produces updated weights in the LoRa format.  
   ```bash
   python llama2.py 
   ```

2. **Merge Weights for Ollama**  
   Run the export process (e.g., via an export script) to merge the base model with the updated weights. This step prepares the model for conversion to the Ollama format.  
   ```bash
   python export_for_ollama.sh
   ```
   

3. **Convert to GGUF Format**  
   Convert the merged model into a gguf model compatible with Ollama by running:
   ```bash
   python ~/llama.cpp/convert_hf_to_gguf.py --outfile ai-terminal/ai-terminal/FineTuned/ollama_model/gguf/opt_1.3b_f16.gguf ai-terminal/ai-terminal/FineTuned/ollama_model/merged_model
   ```

