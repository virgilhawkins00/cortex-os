import os
import sys

name = os.getenv("ARG_NAME", "World")
print(f"Hello, {name}! This is a dynamic script tool.")
