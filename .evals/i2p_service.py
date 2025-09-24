import os
import json
import subprocess
from pathlib import Path
from typing import Optional

import requests
from fastapi import FastAPI, Request, HTTPException

APP_PORT = int(os.getenv("I2P_SERVICE_PORT", "8081"))
REPO_ROOT = Path(os.getenv("REPO_ROOT", "/app")).resolve()
I2P_DIR = REPO_ROOT / ".evals"
OUTPUT_FILE = I2P_DIR / "i2p_dynamic_prompt.md"
OPENROUTER_API_KEY = os.getenv("OPENROUTER_API_KEY", "")
LINEAR_API_KEY = os.getenv("LINEAR_API_KEY", "")
LINEAR_API_URL = os.getenv("LINEAR_API_URL", "https://api.linear.app/graphql")
POST_BACK = os.getenv("LINEAR_POST_BACK", "true").lower() in {"1", "true", "yes"}

app = FastAPI(title="Arda i2p Service", version="1.0.0")


def run_i2p(issue_text: str) -> str:
    I2P_DIR.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    if OPENROUTER_API_KEY:
        env["OPENROUTER_API_KEY"] = OPENROUTER_API_KEY
    cmd = ["python3", "i2p.py", issue_text, "--llm"]
    result = subprocess.run(
        cmd,
        cwd=str(I2P_DIR),
        env=env,
        capture_output=True,
        text=True,
        timeout=600,
    )
    if result.returncode != 0:
        raise RuntimeError(f"i2p failed: {result.stderr or result.stdout}")
    if not OUTPUT_FILE.exists():
        raise RuntimeError("i2p output file not found")
    return OUTPUT_FILE.read_text(encoding="utf-8")


def linear_comment(issue_id: str, body: str) -> Optional[str]:
    if not LINEAR_API_KEY or not POST_BACK:
        return None
    headers = {
        "Authorization": f"Bearer {LINEAR_API_KEY}",
        "Content-Type": "application/json",
    }
    mutation = (
        "mutation($input: IssueCommentCreateInput!) { "
        "issueCommentCreate(input: $input) { success issueComment { id } } }"
    )
    variables = {"input": {"issueId": issue_id, "body": body[:50000]}}
    resp = requests.post(
        LINEAR_API_URL,
        headers=headers,
        json={"query": mutation, "variables": variables},
        timeout=30,
    )
    if resp.status_code != 200:
        raise HTTPException(status_code=502, detail=f"Linear API error: {resp.text}")
    data = resp.json()
    if data.get("errors"):
        raise HTTPException(status_code=502, detail=str(data["errors"]))
    return data.get("data", {}).get("issueCommentCreate", {}).get("issueComment", {}).get("id")


@app.get("/health")
async def health():
    return {"status": "ok", "repo_root": str(REPO_ROOT)}


@app.post("/linear/webhook")
async def linear_webhook(request: Request):
    try:
        payload = await request.json()
    except Exception:
        payload = json.loads((await request.body()).decode("utf-8"))

    # Support common shapes
    data = payload.get("data", payload)
    issue_id = data.get("id") or data.get("issueId")
    title = data.get("title") or ""
    description = data.get("description") or data.get("body") or ""

    if not title and not description:
        raise HTTPException(status_code=400, detail="Missing title/description in webhook payload")

    issue_text = (title + ":\n\n" + description).strip()

    augmented = run_i2p(issue_text)

    comment_id = None
    if issue_id:
        try:
            comment_id = linear_comment(issue_id, augmented)
        except HTTPException as e:
            # Pass through
            raise e
        except Exception:
            # Ignore post-back failures
            comment_id = None

    return {
        "ok": True,
        "issue_id": issue_id,
        "augmented_len": len(augmented),
        "comment_id": comment_id,
    }


# For local debug: uvicorn .evals.i2p_service:app --host 0.0.0.0 --port 8081
