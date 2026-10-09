---
description: >-
  Put a workspace folder on the internet: deploy a site, provision and wire a
  database, set environment variables, attach a domain, roll back.
icon: cloud-arrow-up
---

# Hosting

The agent can take a folder in your workspace and make it a live site, with a database behind it if the app needs one. Ask for it in chat; the deploy itself is a tool call like any other, and an irreversible one goes through the [Approval Gate](approval-gate.md).

{% hint style="info" %}
**Vercel is the only provider today.** The model underneath is provider-neutral, and Railway, Cloudflare and a self-hosted target are the named next steps, but nothing else is wired yet.
{% endhint %}

## What the agent can do

| Tool | Effect |
| --- | --- |
| `hosting_launch_site` | Deploy a workspace directory as a live site, optionally provisioning a database and wiring it in, setting environment variables, and attaching domains. |
| `hosting_deployment_status` | Whether a build has finished. |
| `hosting_list_deployments` | A site's recent deployments, newest first, with status and target. |
| `hosting_deployment_logs` | A deployment's build and runtime log events. |
| `hosting_rollback` | Point production back at an earlier deployment that already built. |
| `hosting_list_sites` | The sites on the account. |
| `hosting_set_env` | Set environment variables on an existing site. |
| `hosting_add_domain` | Attach a custom domain. |
| `hosting_domain_status` | Whether a site's domains are verified and serving. |
| `hosting_analytics` | Traffic over the last N days. |

A launch runs its five steps in the one order that works: create the site, provision the database, **connect the database before the build**, set the environment, then build. A framework that reads its environment at build time gets a broken build in any other order. The call returns while the build is still running, so the agent polls `hosting_deployment_status` rather than blocking.

There is a rollback and no separate promote, because a rollback *is* promoting an older deployment. It refuses a deployment that never finished building, since promoting a failed build would take the site down during an attempt to bring it back up.

## Two things it will not do

- **Read a secret.** A managed database's connection string is injected by the provider into the site's environment. OpenHuman learns the variable *names* and never their values, which is why a launch reports `DATABASE_URL` rather than a URL.
- **Deploy something you did not name.** The directory resolver refuses an absolute path, a `..` escape and anything that is not a directory. It is the one place that decides what may leave the machine, and a deployment uploads every byte under the folder it is given.

## Setting it up

```toml
[hosting]
enabled = true
provider = "vercel"
api_key = ""        # leave blank to read the provider's own env var
team = ""           # blank means your personal account
```

With `api_key` blank the credential comes from `TINYHOSTS_VERCEL_TOKEN`, falling back to `VERCEL_TOKEN`; a team account also reads `TINYHOSTS_VERCEL_TEAM_ID` or `VERCEL_TEAM_ID`.

`[hosting] enabled` is `false` by default. With it off, or with no credential resolvable anywhere, **the ten tools are not registered at all** rather than registered and failing. That is deliberate: a tool that is present and cannot work is worse than one that is absent, because a model retries it. A *misconfigured* section is an error and is logged rather than silently skipped: an unknown provider slug, or a key that is present but blank after trimming. Leaving `api_key = ""` out entirely, or set to the empty string, is the documented way to defer to the provider's environment variable and is not a misconfiguration.

## See also

- [Cloud Deploy](cloud-deploy.md): hosting the OpenHuman core itself, which is a different thing.
- [Approval Gate](approval-gate.md): how a deploy gets your yes.
- [Coder](native-tools/coder.md): building the thing before you ship it.
