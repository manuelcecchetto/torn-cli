# FFScouter support

FFScouter is more than a battle-stat lookup. The official API docs at <https://ffscouter.com/api-docs> expose several useful groups:

- battle-stat estimates for up to 205 targets per request;
- 6-hour battle-stat history buckets;
- premium flight tracking;
- premium player/faction activity buckets;
- premium faction hit calling;
- target finder filters/presets;
- losses marketplace buyer quote and seller workflows;
- API key status/registration;
- announcements.

## Privacy and safety

FFScouter authenticates with `key=<FFSCOUTER_API_KEY>` in the query string. `torn-cli` redacts this from displayed URLs, cache keys, errors, and response bodies. This matters because `/check-key` and `/register` can echo the key.

Live mutation commands require `--yes`:

- `torn ff hits claim ... --yes`
- `torn ff hits unclaim ... --yes`
- `torn ff hits wipe --yes`
- `torn ff losses seller-claim ... --yes`
- `torn ff losses seller-complete ... --yes`

`register` requires `--agree-to-data-policy`; read FFScouter's homepage/data policy first as required by their docs.

## Key/status

```bash
torn ff check-key --pretty
torn ff status --pretty
torn ff register --agree-to-data-policy --signup-source torncli
```

`status` is an alias for `check-key`. The response reports registration, policy version, premium entitlement, faction id, and premium expiry fields.

## Battle stats

```bash
torn ff stats --target 123456 --pretty
torn ff stats --target 123456,789012 --json
```

The API uses `targets`, not `user_id`, and supports up to 205 target IDs in one request. Responses include fair-fight value, battle-stat estimate, public BSS value, source, and premium distribution fields when available.

History:

```bash
torn ff stats-history --target 123456 --since 30d --to now --limit 20 --sort desc --json
```

History is summarized into 6-hour median-smoothed buckets and is limited to 100 buckets by FFScouter.

## Flights and activity premium features

```bash
torn ff flights --target 123456 --pretty

torn ff activity player --target 123456 --since 24h --to now --bucket 900 --json
torn ff activity faction --faction 89 --since 24h --to now --bucket 3600 --json
```

Activity bucket sizes must be `300`, `900`, or `3600` seconds. Player activity is a 0/1 bucket score; faction activity reports active member counts/ratios.

## Target finder

```bash
torn ff targets --preset respect --limit 25 --json
torn ff targets --preset level --limit 25 --json
torn ff targets --min-level 20 --max-level 50 --min-ff 1.5 --max-ff 2.5 --inactive-only --limit 25
torn ff targets --factionless --limit 20
```

When `--preset` is used, FFScouter only accepts `key` and `limit`; `torn-cli` rejects preset + custom filter combinations locally.

## Hit calling premium features

```bash
torn ff hits claims --pretty
torn ff hits claim --target 555111 --yes
torn ff hits unclaim --target 555111 --yes
torn ff hits unclaim --claim-id <uuid> --yes
torn ff hits wipe --yes
```

Hit calling is faction-visible state. Claims expire according to your faction configuration. Only you can remove your own claims.

## Losses marketplace

Buyer-side quote does not use the FFScouter API key:

```bash
torn ff losses quote --quantity 10 --price-per-loss 300000 --pretty
```

Seller-side routes require the configured FFScouter key:

```bash
torn ff losses seller-contracts --pretty
torn ff losses seller-claims --pretty
torn ff losses seller-order --order <order-number> --pretty
torn ff losses seller-claim --order <order-number> --slots 10 --yes
torn ff losses seller-complete --claim-id <claim-id> --yes
```

The buyer order progress endpoint uses a separate 14-character order secret as a `key` query parameter. It is intentionally not wrapped yet because `torn-cli` blocks user-supplied `key=` query parameters to avoid credential leaks. Use the website monitor URL until a dedicated secret-handling flow is added.

## Raw access

Any documented path can still be called directly:

```bash
torn ff get /activity/player --param target=123456 --param start=1770000000 --param end=1770086400
torn ff post /hit-calling/claim --body '{"target_player_id":555111}'
```

Do not pass `key=` manually; configure the key with `torn config set ffscouter-api-key`.
