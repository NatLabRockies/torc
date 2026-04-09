#!/usr/bin/env python3
"""Training script with SIGTERM-aware checkpointing.

Saves periodic checkpoints during training. On SIGTERM (e.g., torc approaching
a time limit), saves an emergency checkpoint and exits cleanly. On restart,
resumes from the latest checkpoint automatically.

Expected environment variables (set by torc or the calling shell):
    TORC_JOB_NAME  - used to create a per-job checkpoint directory
    MODEL_INDEX    - index suffix for the output model file
"""

import numpy as np
import os
import pickle
import signal
import sys
import time

# ── Configuration ──────────────────────────────────────────────────
ckpt_dir = f"/workspace/checkpoints/{os.environ['TORC_JOB_NAME']}"
model_out = f"/workspace/models/model_{os.environ['MODEL_INDEX']}.pt"
os.makedirs(ckpt_dir, exist_ok=True)

total_epochs = 100

# ── SIGTERM handling ───────────────────────────────────────────────
terminated = False


def handle_sigterm(_signum, _frame):
    global terminated
    terminated = True
    print("SIGTERM received — will save checkpoint and exit after current epoch")


signal.signal(signal.SIGTERM, handle_sigterm)

# ── Resume from checkpoint if available ────────────────────────────
checkpoints = sorted(
    [f for f in os.listdir(ckpt_dir) if f.startswith("checkpoint_")],
    reverse=True,
)
start_epoch = 0
weights = np.random.rand(128, 10) * 0.01

if checkpoints:
    latest = os.path.join(ckpt_dir, checkpoints[0])
    data = np.load(latest, allow_pickle=True).item()
    weights = data["weights"]
    start_epoch = data["epoch"] + 1
    print(f"Resuming from checkpoint at epoch {start_epoch}")
else:
    print("Starting fresh training")

# ── Load dataset ───────────────────────────────────────────────────
with open("/workspace/data/dataset.pkl", "rb") as f:
    dataset = pickle.load(f)

# ── Training loop ──────────────────────────────────────────────────
loss = float("inf")
for epoch in range(start_epoch, total_epochs):
    # Simulate training step
    grad = np.random.randn(*weights.shape) * 0.001
    weights -= grad
    loss = float(np.linalg.norm(grad))

    # Periodic checkpoint every 10 epochs
    if (epoch + 1) % 10 == 0:
        ckpt_path = os.path.join(ckpt_dir, f"checkpoint_{epoch:04d}.npy")
        np.save(ckpt_path, {"weights": weights, "epoch": epoch, "loss": loss})
        print(f"Epoch {epoch+1}/{total_epochs} loss={loss:.6f} [checkpoint saved]")
    else:
        print(f"Epoch {epoch+1}/{total_epochs} loss={loss:.6f}")

    # Check if we received SIGTERM — save and exit gracefully
    if terminated:
        ckpt_path = os.path.join(ckpt_dir, f"checkpoint_{epoch:04d}.npy")
        np.save(ckpt_path, {"weights": weights, "epoch": epoch, "loss": loss})
        print(f"Emergency checkpoint saved at epoch {epoch+1}. Exiting.")
        sys.exit(0)

    time.sleep(1)  # Simulate compute time

# Save final model
np.save(model_out, {"weights": weights, "final_loss": loss})
print(f"Training complete. Model saved to {model_out}")
