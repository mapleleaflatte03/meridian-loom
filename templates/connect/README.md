# Connect Templates

Operator-first adapter templates for the priority connect transports:

- `telegram.sample.json`
- `discord.sample.json`
- `browser.sample.json`
- `shell.sample.json`
- `webhook.sample.json`

These are contract-shape examples for `loom connect scaffold|validate|enable|test|health`.
Operator retention + KPI surfaces:

- `loom connect metrics --adapter-id ...`
- `loom connect scorecard`
- `loom connect prune --adapter-id ...`

They do not include provider-specific defaults or secrets.
