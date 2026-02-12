# Arcaferry Python Sidecar (Camoufox)

This directory provides an **optional** Python sidecar used by Arcaferry (Rust) to extract **Quack/Purrly hidden settings** via browser automation.

Arcaferry will work without this sidecar; when the sidecar is missing, the server should **degrade gracefully** and return warnings.

## Install

```bash
pip install -r scripts/requirements.txt
python -m playwright install firefox
```

## CLI usage

The script prints a JSON array to **stdout**:

```json
[
  {"label": "HiddenLabel1", "value": "...", "isVisible": false},
  {"label": "HiddenLabel2", "value": "...", "isVisible": false}
]
```

Example:

```bash
python3 scripts/extract_hidden.py \
  --url "https://purrly.ai/discovery/share/<share_id>" \
  --labels '["HiddenLabel1","HiddenLabel2"]' \
  --cookies "cf_clearance=...; ..." \
  --token "<bearer token>"
```

Notes:
- `--labels` must be a **JSON array of strings**.
- `--cookies` is an optional cookie header string.
- `--token` is an optional Bearer token (without the `Bearer ` prefix is OK).
