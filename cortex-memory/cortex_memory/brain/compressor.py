"""Token Compressor — Caveman-style text reduction.

Reduces token usage by removing English stop words, unnecessary 
articles, and collapsing whitespace, without losing semantic meaning.
Inspired by 'why use many token when few token do trick'.
"""

import re

# Curated list of high-frequency stop words and articles that LLMs can infer
_STOP_WORDS = {
    "a", "an", "the", "and", "but", "if", "or", "because", "as", "what",
    "which", "this", "that", "these", "those", "then", "just", "so", "than",
    "such", "both", "through", "about", "for", "is", "of", "while", "during",
    "to", "from", "in", "out", "on", "off", "over", "under", "again", "further",
    "then", "once", "here", "there", "when", "where", "why", "how", "all", "any"
}

def caveman_compress(text: str) -> str:
    """Compress text by removing stop words and normalizing whitespace.
    
    This function processes the text word by word, lowering the token
    count significantly while remaining fully comprehensible to LLMs.
    """
    if not text:
        return text
        
    # Split by word boundaries keeping punctuation attached where necessary
    # A simple regex to split into words and non-words
    tokens = re.findall(r'\b\w+\b|\S+', text)
    
    compressed_tokens = []
    for token in tokens:
        # Check pure alphabetic words against the stop words list
        if token.isalpha() and token.lower() in _STOP_WORDS:
            continue
        compressed_tokens.append(token)
        
    # Reassemble and collapse whitespace
    compressed_text = " ".join(compressed_tokens)
    
    # Clean up double spaces that might occur around punctuation
    compressed_text = re.sub(r'\s+([.,!?;:])', r'\1', compressed_text)
    compressed_text = re.sub(r'\s+', ' ', compressed_text).strip()
    
    return compressed_text

class TokenBudget:
    """Manages heuristic token budgets for aggressive truncation."""
    
    @staticmethod
    def estimate_tokens(text: str) -> int:
        """Estimate tokens using a rough 1 token = 4 characters heuristic."""
        return len(text) // 4
        
    @classmethod
    def fit_to_budget(cls, text: str, max_tokens: int, compress_first: bool = True) -> str:
        """Fit a text to a max budget, optionally compressing it first.
        
        If it's still too long after compression, we truncate the middle
        to preserve the start (headers) and end (conclusions/errors).
        """
        if compress_first:
            text = caveman_compress(text)
            
        current_est = cls.estimate_tokens(text)
        if current_est <= max_tokens:
            return text
            
        # If it's too long, we keep 40% front, 40% back
        max_chars = max_tokens * 4
        front_chars = int(max_chars * 0.4)
        back_chars = int(max_chars * 0.4)
        
        return f"{text[:front_chars]} ... [TRUNCATED DUE TO TOKEN BUDGET] ... {text[-back_chars:]}"
