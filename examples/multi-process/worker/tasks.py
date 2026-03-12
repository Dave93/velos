"""Minimal background worker that simulates task processing."""

import os
import signal
import sys
import time

running = True


def handle_sigterm(signum, frame):
    global running
    print("Worker: SIGTERM received, finishing current task...")
    running = False


signal.signal(signal.SIGTERM, handle_sigterm)


def process_task(task_id):
    print(f"Processing task #{task_id} (PID: {os.getpid()})")
    time.sleep(2)  # simulate work
    print(f"Task #{task_id} completed")


if __name__ == "__main__":
    print(f"Worker started (PID: {os.getpid()})")
    task_id = 0
    while running:
        task_id += 1
        process_task(task_id)
        time.sleep(1)

    print("Worker shut down gracefully")
    sys.exit(0)
