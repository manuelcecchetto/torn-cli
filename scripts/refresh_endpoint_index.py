#!/usr/bin/env python3
"""Refresh the bundled Torn/FFScouter endpoint index.

The Torn OpenAPI endpoint blocks default user agents in some environments, so this
script sends a descriptive User-Agent. It writes a compact endpoint index used by
`torn endpoints` and by shortcut command resolution; it intentionally does not
vendor full response schemas.
"""
from __future__ import annotations

import argparse
import json
import re
import sys
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

OPENAPI_URL = "https://www.torn.com/swagger/openapi.json"
USER_AGENT = "torn-cli-endpoint-index/0.1 (+https://github.com/manuelcecchetto/torn-cli)"

FF_ENDPOINTS = [
    {"method": "GET", "path": "/check-key", "group": "key", "selection": "check-key", "summary": "Validate FFScouter API key", "parameters": []},
    {"method": "POST", "path": "/register", "group": "key", "selection": "register", "summary": "Register key/user mapping", "parameters": [{"name": "agree_to_data_policy", "in": "body", "required": True}, {"name": "signup_source", "in": "body", "required": True}]},
    {"method": "GET", "path": "/get-stats", "group": "stats", "selection": "stats", "summary": "Current stat snapshot for up to 205 targets", "parameters": [{"name": "targets", "in": "query", "required": True}]},
    {"method": "GET", "path": "/get-stats-history", "group": "stats", "selection": "stats-history", "summary": "Historical battle-stat estimate buckets", "parameters": [{"name": "target", "in": "query", "required": True}, {"name": "limit", "in": "query", "required": False}, {"name": "from", "in": "query", "required": False}, {"name": "to", "in": "query", "required": False}, {"name": "sort", "in": "query", "required": False}]},
    {"method": "GET", "path": "/player-flights", "group": "travel", "selection": "flights", "summary": "Premium player flight/travel data", "parameters": [{"name": "target", "in": "query", "required": True}]},
    {"method": "GET", "path": "/activity/player", "group": "activity", "selection": "player", "summary": "Premium player activity buckets", "parameters": [{"name": "target", "in": "query", "required": True}, {"name": "start", "in": "query", "required": True}, {"name": "end", "in": "query", "required": True}, {"name": "bucket", "in": "query", "required": False}]},
    {"method": "GET", "path": "/activity/faction", "group": "activity", "selection": "faction", "summary": "Premium faction activity buckets", "parameters": [{"name": "faction_id", "in": "query", "required": True}, {"name": "start", "in": "query", "required": True}, {"name": "end", "in": "query", "required": True}, {"name": "bucket", "in": "query", "required": False}]},
    {"method": "GET", "path": "/hit-calling/claims", "group": "hit-calling", "selection": "claims", "summary": "Premium active faction hit claims", "parameters": []},
    {"method": "POST", "path": "/hit-calling/claim", "group": "hit-calling", "selection": "claim", "summary": "Premium claim a target for faction hit calling", "parameters": [{"name": "target_player_id", "in": "body", "required": True}]},
    {"method": "POST", "path": "/hit-calling/unclaim", "group": "hit-calling", "selection": "unclaim", "summary": "Premium release hit-calling claim(s)", "parameters": [{"name": "target_player_id", "in": "body", "required": False}, {"name": "claim_id", "in": "body", "required": False}]},
    {"method": "POST", "path": "/hit-calling/wipe", "group": "hit-calling", "selection": "wipe", "summary": "Premium release every hit-calling claim you placed", "parameters": []},
    {"method": "GET", "path": "/losses/orders/quote", "group": "losses", "selection": "quote", "summary": "Losses marketplace buyer quote", "parameters": [{"name": "quantity", "in": "query", "required": True}, {"name": "price_per_loss", "in": "query", "required": True}]},
    {"method": "GET", "path": "/losses/seller/contracts", "group": "losses", "selection": "seller-contracts", "summary": "Losses marketplace seller contracts", "parameters": []},
    {"method": "GET", "path": "/losses/seller/claims", "group": "losses", "selection": "seller-claims", "summary": "Losses marketplace seller claims", "parameters": []},
    {"method": "GET", "path": "/losses/seller/orders/{orderNumber}", "group": "losses", "selection": "seller-order", "summary": "Losses marketplace seller order visibility", "parameters": [{"name": "orderNumber", "in": "path", "required": True}]},
    {"method": "POST", "path": "/losses/seller/claim", "group": "losses", "selection": "seller-claim", "summary": "Losses marketplace reserve seller slots", "parameters": [{"name": "order_number", "in": "body", "required": True}, {"name": "slots", "in": "body", "required": False}]},
    {"method": "POST", "path": "/losses/seller/claims/{id}/complete", "group": "losses", "selection": "seller-complete", "summary": "Losses marketplace mark seller claim complete", "parameters": [{"name": "id", "in": "path", "required": True}]},
    {"method": "GET", "path": "/get-targets", "group": "targets", "selection": "targets", "summary": "Target scouting list", "parameters": [{"name": "preset", "in": "query", "required": False}, {"name": "minlevel", "in": "query", "required": False}, {"name": "maxlevel", "in": "query", "required": False}, {"name": "inactiveonly", "in": "query", "required": False}, {"name": "minff", "in": "query", "required": False}, {"name": "maxff", "in": "query", "required": False}, {"name": "limit", "in": "query", "required": False}, {"name": "factionless", "in": "query", "required": False}]},
    {"method": "GET", "path": "/announcements", "group": "announcements", "selection": "announcements", "summary": "Announcement feed", "parameters": []},
]


def load_openapi(url: str) -> dict[str, Any]:
    req = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(req, timeout=60) as resp:  # noqa: S310 - pinned public API source
        return json.load(resp)


def resolve_ref(doc: dict[str, Any], value: dict[str, Any]) -> dict[str, Any]:
    ref = value.get("$ref")
    if not ref:
        return value
    if not ref.startswith("#/"):
        return value
    cur: Any = doc
    for part in ref[2:].split("/"):
        cur = cur[part]
    return cur


def schema_kind(schema: dict[str, Any] | None) -> tuple[str | None, bool, list[str]]:
    if not schema:
        return None, False, []
    if "$ref" in schema:
        name = schema["$ref"].rsplit("/", 1)[-1]
        return name, False, []
    if schema.get("type") == "array":
        item = schema.get("items", {})
        if "$ref" in item:
            return item["$ref"].rsplit("/", 1)[-1], True, []
        return item.get("type"), True, item.get("enum") or []
    if "oneOf" in schema:
        labels = []
        for item in schema["oneOf"]:
            if "$ref" in item:
                labels.append(item["$ref"].rsplit("/", 1)[-1])
            elif item.get("type"):
                labels.append(item["type"])
        return " | ".join(labels), False, []
    return schema.get("type"), False, schema.get("enum") or []


def selection_from_path(path: str) -> str | None:
    parts = [p for p in path.split("/") if p]
    if len(parts) == 1:
        return None
    if len(parts) >= 3 and re.fullmatch(r"\{[^}]+\}", parts[1]):
        return parts[2]
    return parts[1]


def path_params(path: str) -> list[str]:
    return re.findall(r"\{([^}]+)\}", path)


def clean_description(text: str | None) -> str | None:
    if not text:
        return None
    return " ".join(re.sub(r"<[^>]+>", " ", text).split())


def endpoint_from_operation(doc: dict[str, Any], path: str, method: str, op: dict[str, Any]) -> dict[str, Any]:
    group = path.split("/")[1]
    params = []
    auth_level = None
    for raw in op.get("parameters", []):
        p = resolve_ref(doc, raw)
        name = p.get("name")
        if not name:
            continue
        typ, is_array, enum_values = schema_kind(p.get("schema"))
        item = {
            "name": name,
            "in": p.get("in"),
            "required": bool(p.get("required")),
            "description": clean_description(p.get("description")),
            "schema": typ,
            "array": is_array,
            "secret": name.lower() in {"key", "api_key", "apikey"},
        }
        if enum_values:
            item["enum"] = enum_values
        params.append(item)
        if name == "key" and p.get("description"):
            m = re.search(r"API key \(([^)]+)\)", p["description"])
            if m:
                auth_level = m.group(1)
    return {
        "method": method.upper(),
        "path": path,
        "group": group,
        "selection": selection_from_path(path),
        "path_params": path_params(path),
        "operation_id": op.get("operationId"),
        "summary": clean_description(op.get("summary")),
        "description": clean_description(op.get("description")),
        "stability": op.get("x-stability"),
        "auth_level": auth_level,
        "parameters": params,
    }


def build_index(doc: dict[str, Any]) -> dict[str, Any]:
    endpoints = []
    for path, item in doc.get("paths", {}).items():
        for method, op in item.items():
            if method.lower() != "get":
                continue
            endpoints.append(endpoint_from_operation(doc, path, method, op))
    endpoints.sort(key=lambda e: (e["group"], e["selection"] or "", len(e["path_params"]), e["path"]))

    groups: dict[str, dict[str, Any]] = {}
    for endpoint in endpoints:
        g = groups.setdefault(endpoint["group"], {"name": endpoint["group"], "selections": [], "endpoints": []})
        if endpoint["selection"] and endpoint["selection"] not in g["selections"]:
            g["selections"].append(endpoint["selection"])
        g["endpoints"].append(endpoint)
    for group in groups.values():
        group["selections"].sort(key=str.lower)

    return {
        "schema_version": 1,
        "generated_at": datetime.now(timezone.utc).replace(microsecond=0).isoformat(),
        "generated_from": OPENAPI_URL,
        "openapi_version": doc.get("openapi"),
        "api_version": doc.get("info", {}).get("version"),
        "services": {
            "torn": {
                "base_url": "https://api.torn.com/v2",
                "auth": "Authorization: ApiKey <TORN_API_KEY>",
                "endpoint_count": len(endpoints),
                "groups": [groups[name] for name in sorted(groups)],
            },
            "ffscouter": {
                "base_url": "https://ffscouter.com/api/v1",
                "auth": "query parameter key=<FFSCOUTER_API_KEY>",
                "endpoint_count": len(FF_ENDPOINTS),
                "groups": [
                    {"name": name, "selections": sorted({e["selection"] for e in FF_ENDPOINTS if e["group"] == name}), "endpoints": [e for e in FF_ENDPOINTS if e["group"] == name]}
                    for name in sorted({e["group"] for e in FF_ENDPOINTS})
                ],
            },
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", default="assets/endpoint-index.json")
    parser.add_argument("--input", help="Use a local OpenAPI JSON file instead of fetching")
    args = parser.parse_args()

    if args.input:
        with open(args.input, "r", encoding="utf-8") as fh:
            doc = json.load(fh)
    else:
        doc = load_openapi(OPENAPI_URL)
    index = build_index(doc)
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(index, indent=2, sort_keys=False) + "\n", encoding="utf-8")
    print(f"wrote {output} with {index['services']['torn']['endpoint_count']} Torn endpoints")
    return 0


if __name__ == "__main__":
    sys.exit(main())
