import os
import time
import signal
import sys

from fastapi import FastAPI

app = FastAPI(title="Velos Example — FastAPI")

start_time = time.time()


@app.get("/")
def root():
    return {
        "message": "Hello from Velos!",
        "pid": os.getpid(),
        "env": os.getenv("APP_ENV", "default"),
    }


@app.get("/health")
def health():
    return {"status": "ok", "uptime": round(time.time() - start_time, 2)}


def handle_sigterm(signum, frame):
    print("SIGTERM received, shutting down gracefully...")
    sys.exit(0)


signal.signal(signal.SIGTERM, handle_sigterm)

if __name__ == "__main__":
    import uvicorn

    host = os.getenv("HOST", "0.0.0.0")
    port = int(os.getenv("PORT", "8000"))
    uvicorn.run(app, host=host, port=port)
