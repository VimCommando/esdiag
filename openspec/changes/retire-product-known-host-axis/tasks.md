## 1. Domain model and compatibility boundary

- [ ] 1.1 Add an ADR for saved-host classification and update `CONTEXT.md` with saved host, route, unresolved host, and resolved host terminology.
- [ ] 1.2 Introduce the persisted-record to resolved-host boundary so runtime hosts always carry a concrete URL and one API-collectable `Application`.
- [ ] 1.3 Represent direct and recognized Elastic Cloud admin routing independently from target application and preserve the receiver's `ElasticCloudHosted` platform hint.
- [ ] 1.4 Centralize resolution of concrete records and template references, including URL rendering, application selection/inference, route derivation, and role validation.
- [ ] 1.5 Add tolerant legacy `app` deserialization for application, unknown/none, and platform-only values without adding `Unknown` to `Application`.
- [ ] 1.6 Normalize current `hosts.yml` serialization so classified records write only `Application` values and unresolved dynamic templates omit `app`.

## 2. Saved-host surfaces

- [ ] 2.1 Move CLI host add, update, list, auth, and materialized-template flows onto the centralized host resolution boundary.
- [ ] 2.2 Restrict CLI `--app` and template product selection to supported applications and provide actionable errors for platforms or ambiguous legacy records.
- [ ] 2.3 Update Web host form state, rows, and backend handlers to distinguish unresolved templates from concrete classified hosts without fake application values.
- [ ] 2.4 Ensure CLI and Web connection/authentication tests accept only resolved hosts and return template-resolution guidance otherwise.
- [ ] 2.5 Add CLI and Web tests for direct hosts, Cloud admin routes, unresolved templates, materialization, forged platform values, and ambiguous legacy records.

## 3. Role and runtime dispatch migration

- [ ] 3.1 Validate `collect`, `send`, and `view` roles against resolved `Application`, deferring dynamic-template checks until materialization and checking fixed templates at save time.
- [ ] 3.2 Replace `Product` in URI, client, and receiver selection with `Application` plus independent route metadata.
- [ ] 3.3 Replace `Product` in collectors, collect options, source-registry selection, and archive naming with the API-collectable `Application` subset.
- [ ] 3.4 Replace remaining application-specific `Product` use in job execution, setup, exporter, server, saved-job, and helper call sites.
- [ ] 3.5 Preserve the existing Collect scope: Elasticsearch, Kibana, and Logstash are live-collectable; Agent and platform diagnostics remain Load-only.
- [ ] 3.6 Add exhaustive dispatch tests covering each collectable application through valid direct and Cloud admin routes and rejecting incompatible combinations.

## 4. Manifest wire isolation

- [ ] 4.1 Introduce a manifest-local compatibility representation for the legacy `product` field and its complete historical value set.
- [ ] 4.2 Convert manifest wire classification to `Platform` plus optional `Application` at the manifest boundary without exporting the compatibility type as a domain model.
- [ ] 4.3 Update diagnostic manifest, included-diagnostic, data-source, and receiver manifest paths to use the isolated wire type or orthogonal classifications as appropriate.
- [ ] 4.4 Add fixture and round-trip tests proving historical manifests still deserialize and the `product` field name, shape, and accepted values remain unchanged.

## 5. Retire the alias

- [ ] 5.1 Remove `data::Product`, its module export, conversion helpers, compatibility builder methods, and tests that exercise the flattened domain type.
- [ ] 5.2 Replace test fixtures and constructors with `Application`, `Platform`, resolved-host builders, or the manifest-local wire type according to the axis under test.
- [ ] 5.3 Verify no general `Product` type or `Product::` references remain and no platform or route value is accepted as a host application.

## 6. Documentation and release note

- [ ] 6.1 Update nearby CLI, Web, saved-host, and maintainer documentation for application/route separation, unresolved templates, and legacy-host correction guidance.
- [ ] 6.2 Add a `CHANGELOG.md` entry for the user-visible saved-host normalization and validation changes using the repository changelog skill.
- [ ] 6.3 Cross-reference issue 366 and note that the overlapping issue 304 template schema remains behaviorally compatible.

## 7. Verification

- [ ] 7.1 Run focused unit and integration tests for known hosts, URI resolution, receivers, collectors, manifests, CLI host commands, and Web host management.
- [ ] 7.2 Run `cargo fmt --all -- --check`.
- [ ] 7.3 Run `cargo clippy` for the repository's supported feature configurations and resolve all warnings.
- [ ] 7.4 Run `cargo test` for the repository's supported feature configurations.
- [ ] 7.5 Run OpenSpec validation and confirm every scenario in the four delta specifications has test coverage.
