# Releasing

Publishing runs from `release.yml`. It is driven by a tag and it authenticates
with a short-lived crates.io token obtained over OIDC. There is no
`CARGO_REGISTRY_TOKEN` secret in this repository.

## The version rule

The tag is `v` plus the version in the manifests. Tag `v0.3.1` publishes the
crates only if `gotmpl/Cargo.toml` and `sprig/Cargo.toml` both say
`version = "0.3.1"`. The `version` job fails the run when they differ from each
other or from the tag, so a tag can never publish a version that differs from
what is in the tree.

This repository ships two crates that share the tag: `neotales-sprig` from
`sprig/` and `neotales-gotmpl` from `gotmpl/`. `neotales-gotmpl`'s published
graph names nothing: the engine has no dependency on a helper set. `sprig/` names
`neotales-gotmpl` as an **optional** dependency, which the `gotmpl` feature turns
on, so a default build of the helper crate still names no engine.

That one optional dependency is versioned, so packaging `sprig/` resolves
`neotales-gotmpl` from crates.io. Two consequences:

- `neotales-gotmpl` is published first. Its only edge to `sprig/` is a path-only
  dev-dependency, which cargo leaves out of the packaged manifest, so it does not
  need the helper crate to exist.
- `neotales-sprig --dry-run` fails with `no matching package named
  neotales-gotmpl` until that exact version is on crates.io. The `check` job
  therefore dry-runs gotmpl only, and the publish job dry-runs sprig after it
  uploads gotmpl, immediately before uploading sprig.

Bump the version, commit, push, then tag that commit:

```sh
cargo publish -p neotales-gotmpl --dry-run --locked
cargo publish -p neotales-sprig --dry-run --locked   # only once gotmpl's version is on crates.io
git tag v0.3.1
git push origin dev
git push origin v0.3.1
```

The tag uploads both crates, in this order, and neither upload is optional:

```sh
cargo publish -p neotales-gotmpl --locked
cargo publish -p neotales-sprig --locked
```

The tag run publishes gotmpl, then sprig. A second upload of the gotmpl version
is skipped, so a run that failed on sprig — the usual cause is crates.io index
lag between the two uploads — can be re-run as soon as the index lists gotmpl.
Sprig is not skipped: a version that is already uploaded is an error, not a
no-op.

The one exception is a new crate's first version, which is uploaded by hand and
never from a tag. That is the next section.

## First publish of a new crate

crates.io trusted publishing cannot create a brand-new crate. The OIDC exchange
needs a crate that exists and a trusted publisher configured on it, so a cold
start over OIDC is rejected by crates.io. Do not try to build one.

The first version of a new crate is `0.0.0-alpha.0`. Not `0.0.0-alpha.1` and
not `0.1.0`. The first upload is manual, and a manual upload needs a version
nothing else has claimed.

The first upload goes through `/home/dev/work/rs/publish.sh <dir>`. That script
lives beside this folder, in `~/work/rs`, and is deliberately not copied into
the crate repository: it is how the first version of every crate is uploaded,
not part of any one crate.

1. Set `version = "0.0.0-alpha.0"` in `gotmpl/Cargo.toml` and
   `sprig/Cargo.toml`, commit it, and leave the working tree clean. Do not
   publish from a tree with uncommitted changes.
2. Run the script with each crate's directory, gotmpl first: sprig's optional
   dependency needs that version on crates.io before sprig can be packaged.

   ```sh
   ~/work/rs/publish.sh gotmpl
   ~/work/rs/publish.sh sprig
   ```

   The script reads the token itself and passes it to `cargo` through the
   environment. Do not put a token on the command line, in the repository, or
   in shell history.
3. Add the trusted publisher on crates.io for both crates, in each crate's
   settings:
   owner `neotales`, repository name, workflow filename `release.yml`, and an
   empty environment. Leave the environment field empty. The `publish` job in
   `release.yml` does not set an environment, and crates.io compares the two
   fields: an environment name here that the job does not set, or the reverse,
   fails the token exchange and nothing else fails with it. If someone later
   sets an environment name on crates.io, the job must set that exact name.
4. Bump to the next version and let tags take over. Do not run `publish.sh`
   for this crate again.

The release run for that first version fails, because the version it would
publish is already uploaded. The tag is provenance for the first version, not
its publish trigger. Every later version publishes from its `v*` tag.

If the crate name is taken on crates.io by someone else, stop. A rename is a
different repository setup, not a flag.

`cargo package` includes every tracked file, so `.github/` and `RELEASE.md`
ship inside the published crate unless the manifest excludes them. That is
harmless. Exclude them if package size matters. The README style guide lives
at `~/work/rs/README.STYLE.md` and is not part of a crate.

## Later releases

1. Bump the version in `Cargo.toml` and commit it. `Cargo.lock` is committed on
   purpose, so run `cargo build` and commit the updated lockfile with it. The
   lockfile is what the scheduled audit checks.
2. Push the commit.
3. Tag it `vX.Y.Z` and push the tag.
4. The workflow runs the checks, dry-runs gotmpl, publishes it, dry-runs sprig
   against the published gotmpl, then publishes sprig, all with a token minted
   for that run.

A manual run, `workflow_dispatch`, runs the checks and the dry run only. It
never publishes, whatever branch it is started from.

After the manual first upload is done, switch the crate to Trusted Publishing
Only mode on crates.io. That rejects token publishes everywhere, so a token
cannot publish this crate later.

## What is not allowed

- `pull_request_target` and `workflow_run` are not used anywhere, and must not
  be added. Both can run untrusted code with publishing credentials in scope.
- No long-lived crates.io token in GitHub secrets for this workflow. The job
  reads `id-token: write` and nothing else.
- No publishing from a branch. The tag check exists so the artifact has a name.
- No republishing a version. crates.io rejects it. If a publish fails after the
  upload was accepted, bump the patch version and release again.

## Failure modes

| Symptom | Cause |
| --- | --- |
| `version` job: tag does not match `vX.Y.Z` | `Cargo.toml` version and the tag disagree. Delete the tag, fix, re-tag. |
| Publish job: OIDC exchange fails | The crate does not exist yet, or the trusted publisher does not match: wrong repository, wrong workflow filename, or an environment name that is set on one side and not the other. |
| Publish job: version already uploaded | The version was published before, usually by the manual first release. Bump the version. Gotmpl is skipped instead, so a failed sprig upload can be re-run. |
| Sprig package or publish: `no matching package named neotales-gotmpl` | Sprig's optional `gotmpl` dependency needs that exact version on crates.io. Publish gotmpl first; the publish job does, and skips it when it is already there. |
| Publish job: `cargo publish` rejects the token | Trusted Publishing Only mode is on and the crate has no trusted publisher configured. |
