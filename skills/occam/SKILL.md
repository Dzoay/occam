---
name: occam
description: Build and change CAD models with the occam command while the person watches each change live in occam's viewer page, and act on the faces and edges they select there. Use when asked to model a part with occam, or when the person speaks of what they selected or clicked in the viewer.
---

# Building with occam while the person watches

`occam run --help` is the guide to writing requests, and each command's `--help` says what it takes. This skill says
what the guide does not: how to work with a person who watches the model in occam's viewer and selects in it.

## Run occam alone, and write requests with your file tool

A project may let only `occam` commands run in the shell. So run each `occam` command on its own, with no `&&`, pipe
or redirection, and write each request file with your file-writing tool (Write), never with `cat`, `echo` or a heredoc.
`occam run` takes the request as a file, not inline. If a shell command is refused, that says nothing about `occam`:
run the `occam` command alone.

## Start the viewer, and say where to look

Choose the model's file, here `part.occam`, and start its viewer before the first run:

```sh
occam view --model part.occam
```

It opens the page in a browser, prints the page's address and returns: tell the person to look there. Until a run
writes the model it also says there is no model at `part.occam`, which is expected. The viewer keeps running between
commands, and `occam view` again starts nothing new and prints the same address.

## Build in runs you can check

The page redraws after each run that changes the model, so build in steps the person can follow, and read each result
before the next. Write a request with your file tool, here `block.json`:

```json
{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
  {"op": "face", "label": "outline", "outer": {"points": [[0, 0, 0], [40, 0, 0], [40, 30, 0], [0, 30, 0]], "closed": true}},
  {"op": "extrude", "label": "block", "face": "outline", "vector": [0, 0, 10]}
]}
```

Then run it, naming the file:

```sh
occam run --model part.occam block.json
```

The result names what each step made, such as `body:0/2`. A label names only what its own request made: a later
request names it by its id.

## Read the selection when the person says "the selected"

In the page a click selects a face or an edge, and shift-click adds or removes one. When the person speaks of the
selected face, the one they clicked, or "this", read what they selected:

```sh
occam selection --model part.occam
```

It describes each selected id as `occam describe` does. A flat face gives its plane's `origin` and `normal`, and its
box as `low` and `high`. The centre of a flat face square to the axes is the middle of `low` and `high`, not its
`origin`. Its `normal` may point into the solid: the outside is the side the body's bounds do not reach. If nothing is
selected, ask the person to click what they mean; if the selection is refused as `view.selection-stale`, ask them to
select again.

## Highlight before you change what is selected

Show the person what you are about to act on, by its id, before the run that acts on it:

```sh
occam highlight --model part.occam face:5/1
```

The page draws it in a colour of its own until the next run that changes the model. Here the selected face is the
block's top, `face:5/1`, from `low` about (0, 0, 10) to `high` about (40, 30, 10), so its centre is (20, 15, 10). A pin
of radius 3 and height 8 stands on it with its frame's origin there and its `z` pointing out of the block. Write
`pin.json` with your file tool:

```json
{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
  {"op": "cylinder", "label": "pin", "radius": 3, "height": 8, "frame": {"origin": [20, 15, 10], "z": [0, 0, 1]}}
]}
```

```sh
occam run --model part.occam pin.json
```

This build has no boolean, fillet or chamfer, so a body placed on a face stays a body of its own: say so if the person
asks to join, cut or round.

## Measure to confirm

Check what a run made against what was asked before you say it is done. A request of measures alone leaves the model as
it was. Write `check.json` with your file tool:

```json
{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
  {"op": "measure", "target": "body:1/1", "what": ["volume", "bounds"]}
]}
```

```sh
occam run --model part.occam check.json
```

## Stop the viewer when the person is done

Leave the viewer running between requests. When the person says they are done, stop it:

```sh
occam view --model part.occam --stop
```
