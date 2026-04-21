"""External API Clients for Cortex Memory.

Handles communication with paid LLM APIs (OpenAI, Anthropic, Gemini, Groq)
if the user prefers them over local Ollama execution.

These clients expect the prompt/context to already be compressed by the
TokenBudget system to minimize costs natively.
"""

import os
import json
import logging
from typing import Any
import urllib.request
import urllib.error

logger = logging.getLogger(__name__)


class ExternalApiClient:
    """Base client for external LLM APIs."""
    
    def __init__(self, api_key: str, model_id: str):
        self.api_key = api_key
        self.model_id = model_id

    async def generate(self, prompt: str, system_prompt: str = "") -> str:
        """Generate text from the external API."""
        raise NotImplementedError


class OpenAiClient(ExternalApiClient):
    """Client for OpenAI Chat API (GPT-4o, etc)."""
    
    async def generate(self, prompt: str, system_prompt: str = "") -> str:
        url = "https://api.openai.com/v1/chat/completions"
        headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {self.api_key}"
        }
        
        messages = []
        if system_prompt:
            messages.append({"role": "system", "content": system_prompt})
            
        messages.append({"role": "user", "content": prompt})
        
        data = {
            "model": self.model_id,
            "messages": messages,
            "temperature": 0.2
        }
        
        req = urllib.request.Request(
            url, 
            data=json.dumps(data).encode("utf-8"), 
            headers=headers
        )
        
        try:
            with urllib.request.urlopen(req) as response:
                result = json.loads(response.read().decode())
                return result["choices"][0]["message"]["content"]
        except Exception as e:
            logger.error("OpenAI API error: %s", e)
            return f"Error communicating with OpenAI: {e}"


class AnthropicClient(ExternalApiClient):
    """Client for Anthropic Messages API (Claude 3.5 Sonnet, etc)."""
    
    async def generate(self, prompt: str, system_prompt: str = "") -> str:
        url = "https://api.anthropic.com/v1/messages"
        headers = {
            "Content-Type": "application/json",
            "x-api-key": self.api_key,
            "anthropic-version": "2023-06-01"
        }
        
        data = {
            "model": self.model_id,
            "system": system_prompt,
            "messages": [{"role": "user", "content": prompt}],
            "max_tokens": 4000,
            "temperature": 0.2
        }
        
        req = urllib.request.Request(
            url, 
            data=json.dumps(data).encode("utf-8"), 
            headers=headers
        )
        
        try:
            with urllib.request.urlopen(req) as response:
                result = json.loads(response.read().decode())
                return result["content"][0]["text"]
        except Exception as e:
            logger.error("Anthropic API error: %s", e)
            return f"Error communicating with Anthropic: {e}"


class GeminiClient(ExternalApiClient):
    """Client for Google Gemini API."""
    
    async def generate(self, prompt: str, system_prompt: str = "") -> str:
        url = f"https://generativelanguage.googleapis.com/v1beta/models/{self.model_id}:generateContent?key={self.api_key}"
        headers = {"Content-Type": "application/json"}
        
        full_prompt = f"{system_prompt}\n\n{prompt}" if system_prompt else prompt
        data = {
            "contents": [{"parts": [{"text": full_prompt}]}],
            "generationConfig": {"temperature": 0.2}
        }
        
        req = urllib.request.Request(
            url, 
            data=json.dumps(data).encode("utf-8"), 
            headers=headers
        )
        
        try:
            with urllib.request.urlopen(req) as response:
                result = json.loads(response.read().decode())
                return result["candidates"][0]["content"]["parts"][0]["text"]
        except Exception as e:
            logger.error("Gemini API error: %s", e)
            return f"Error communicating with Gemini: {e}"


class GroqClient(ExternalApiClient):
    """Client for Groq API (High-speed open source models)."""
    
    async def generate(self, prompt: str, system_prompt: str = "") -> str:
        url = "https://api.groq.com/openai/v1/chat/completions"
        headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {self.api_key}"
        }
        
        messages = []
        if system_prompt:
            messages.append({"role": "system", "content": system_prompt})
            
        messages.append({"role": "user", "content": prompt})
        
        data = {
            "model": self.model_id,
            "messages": messages,
            "temperature": 0.2
        }
        
        req = urllib.request.Request(
            url, 
            data=json.dumps(data).encode("utf-8"), 
            headers=headers
        )
        
        try:
            with urllib.request.urlopen(req) as response:
                result = json.loads(response.read().decode())
                return result["choices"][0]["message"]["content"]
        except Exception as e:
            logger.error("Groq API error: %s", e)
            return f"Error communicating with Groq: {e}"
