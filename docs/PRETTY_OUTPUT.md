# Watch and schema-aware pretty output

`torn --watch [interval] ...` repeats GET requests until interrupted. It bypasses cache by default (`CachePolicy::Fresh`) so status changes such as hospital release are visible.

Examples:

```bash
torn --watch 30s --pretty api user basic --id 1844049
torn --watch 10s --table api user profile --id 1844049
torn --watch 1m --pretty ff stats --target 1844049
```

Watch output prefixes every rendered line with a colored local time marker:

```text
[22:19:24] Glasnost [1844049]  level 95
[22:19:24] status  Okay
```

## Pretty-output policy

The Torn OpenAPI snapshot currently exposes 205 GET paths and 647 component schemas. Most response schemas are thin wrappers around a top-level key. `--pretty` now uses the request path and the top-level response key to choose a human summary instead of always dumping pretty JSON.

Schema families studied from the OpenAPI response wrappers:

- `profile` (`/user/basic`, `/user/{id}/basic`, `/user/profile`, `/user/{id}/profile`, company profile): headline plus status, last action, life, role/property/faction fields.
- `members`: member table with id/name/level/position/status/last action/revive/wall flags.
- `bars`: energy/nerve/happy/life `current/maximum` table.
- `cooldowns`: seconds rendered as human durations.
- `travel`: destination/method/departure/arrival/time-left summary.
- `attacks`, `revives`: combat tables with start/end, participants, result, respect/fee/chance fields.
- timeline/list wrappers such as `events`, `messages`, `notifications`, `news`, `log`: timestamp/id/type/title/message columns where present.
- inventory/market/item/property/stock/list wrappers: dynamic table using schema/common field priorities (`id`, `name`, `type`, `quantity`, `price`, `value`, etc.).
- `info` (`/key/info`): key access/user/selection summary.
- `timestamp`: Unix timestamps rendered as UTC date-times.
- lookup wrappers (`selections`) and uncommon one-off schemas: generic key/value or dynamic table fallback.

FFScouter pretty handling covers:

- `/get-stats`: player id, fair fight, battle-stat estimate, source, last updated, premium availability.
- `/get-targets`: target table using id/name/level/fair-fight/status/last-action fields when present.
- `/check-key`: registration/premium/key-status summary with secrets already redacted.
- `/losses/orders/quote`: buyer quote key/value summary with large values grouped.
- history/activity/flights and other FFScouter wrappers: dynamic table/key-value fallback.

Colors are applied for API/FFScouter terminal `--pretty` output and always for watch prefixes/output. Machine-readable output remains available through `--json`, `--raw`, and `--csv`.
