## REMOVED Requirements

### Requirement: One-Job and Two-Job Workflow Modes
**Reason**: The one-/two-job boundary was an artifact of the always-staged legacy path.
Under the unified model a job is a single `Job` whose execution mode (staged vs streaming) is
*derived* from whether `Save` is selected — covered by the "Derived Execution Mode"
requirement `unified-job-model` already landed. There is no second job created when saving.
The requirement can only be removed once the web runner constructs `Job`s and drives them
through the one executor, because until then the saved-bundle path really does run as two
jobs.
**Migration**: `Collect -> Process -> Send` without `Save` is now one **streaming** job;
`Collect -> Save -> Process` (optionally with `Send`) is now one **staged** job. Callers that
previously created a second job to consume the retained archive instead construct a single
staged `Job`; the executor materialises the bundle as the serialization barrier internally.
