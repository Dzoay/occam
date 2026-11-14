# A person prompts an agent, the agent drives the API, and the browser shows the model live

**Decided by the user, 2026-11-14,** while the [`view`](../plan/view.md) milestone was still a draft. The user asked
what occam is aiming at, an agent-first CAD engine, after asking about a live demo of an agent building a bracket:

> I don't think an agent should be driving the UX of the browser to do this, we may not even need connect human based
> edit UX. What I think we need to target is instead an agent prompt that a human interacts with and the agent drives
> the API with the browser showing the human the live results. We will need live human edit tools in the future plan
> for parameterised CAD but the building I think should be prompt driven ideally, driven by cheap models - with an
> agent first API and skills if they are needed for more context of certain workflows.

Three questions followed, each with a recommendation the user took:

- **Where the person types the prompt.** In an agent they already use, with occam's commands as its tools and a page
  that redraws the model as it changes, or in a chat panel in occam's own page. **"Option 1 first, then add the
  in-page chat later".**
- **How the page and the agent meet.** (A) the model file is the meeting point: `occam view` watches it, serves the
  page and keeps the person's selection beside it; (B) an in-memory server that holds the model, with the commands as
  its clients; (C) an MCP server. **"Go with A but I think we are going to need B later and a client server approach
  becomes very interesting if we were to deploy this as a web service in the future with the prompt in the browser
  from a model that is also chosen there - that is a future potential direction though, not for now. NOW we should get
  to the e2e of seeing agentic CAD modelling live - it will be a nice demo."**
- **What the person does in the page.** The standard view controls, and selection that the agent acts on: "human
  selects an edge and tells the agent, round the selected edge. Human selects a face (or whatever is necessary) and
  then asks the agent to extrude it." Editing the model by hand waits for the editing tools
  ([write-through moves with them](2026-11-14-write-through-after-the-first-release.md)).

## What it means in practice

This section is Claude's working reading of the decision. The user can change it. The design is
[the live view](../architecture/live-view.md).

- **The agent's API is the `occam` command**, the one the [construction blind run](../evidence/2026-11-13-construction-blind-run.md)
  drove with Haiku. It does not change shape for the page: building stays `occam run`, and the page adds `occam view`,
  `occam selection` and `occam highlight`.
- **The page is a window, not a control panel.** It draws the model each time a run saves it, orbits, pans, zooms,
  centres on the selection and fits the model to the window. A person selects faces and edges in it; an agent reads the
  selection and can highlight what it is about to change. No agent drives the page.
- **Skills only where the command's own guide is not enough.** The first skill lands with `view`: start the viewer,
  read the selection, highlight before a change.
- **Cheap models first.** The milestone's agent test runs on the cheapest model the blind runs used, as they did.
- **The order: the live demo now.** `view` reaches the end-to-end demo first, then hardens the mesher and times it.
  Rounding an edge needs [`blends`](../plan/first-release.md), and extruding a face of a solid adds material to it, so
  it needs `booleans`; until then the demo builds, selects, describes, measures and places new bodies on what is
  selected.
- **Later, each after the first release and with the user's go-ahead:** an in-memory session server (B), which a web
  service would grow from, with the prompt and the choice of model in the browser; the in-page chat; an MCP server,
  only if measured agent runs show that cheap models need typed tools.
