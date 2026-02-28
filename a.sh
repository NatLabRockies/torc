#!/bin/bash

curl -X POST "https://aif-prod-eastus2-naerm-001.services.ai.azure.com/anthropic/v1/messages" \
  -H "Content-Type: application/json" \
  -H "x-api-key: mykey" \
  -H "anthropic-version: 2023-06-01" \
  -d '{
    "max_tokens": 1000,
    "temperature": 0.7,
    "system": "You are a helpful assistant.",
    "messages": [
      {
        "role": "user",
        "content": "What are 3 things to visit in Seattle?"
      }
    ],
    "model": "claude-opus-4-5"
  }'
