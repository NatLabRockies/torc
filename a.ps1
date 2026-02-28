$headers = @{
    "Content-Type"      = "application/json"
    "x-api-key"         = "mykey"
    "anthropic-version" = "2023-06-01"
}

# Next, create the body of the request as a PowerShell object.
# I've corrected the JSON structure for you. The "model" key should be inside the main object.
$body = @{
    "max_tokens"  = 1000
    "temperature" = 0.7
    "system"      = "You are a helpful assistant."
    "messages"    = @(
        @{
            "role"    = "user"
            "content" = "What are 3 things to visit in Seattle?"
        }
    )
    "model"       = "claude-opus-4-5"
} | ConvertTo-Json

# Finally, make the request
Invoke-RestMethod -Uri "https://aif-prod-eastus2-naerm-001.services.ai.azure.com/anthropic/v1/messages" -Method POST -Headers $headers -Body $body
