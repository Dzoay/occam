# The brief

The user's requirements, verbatim. They are the contract: nothing else in this repository overrides them, and they are
changed only by the user.

## The user's words, 2026-11-08

> I would like to create a plan to build a rust based CAD engine. I am thinking of taking open cascade that FreeCAD is
> built on and doing the same to that as we did in phasekit which is a rewrite of coolprop. We should take the lessons
> we learned from the Phasekit rewrite and apply it to this new project. All the good practices in phasekit should come
> over, SOLID, DRY, layered code with clear modular responsibilities, extensible, clear unit system etc., test driven
> with mutation testing. This is about creating a clean, simple, FAST library that we then build a web based renderer
> on top of (unlike FreeCADs QT based renderer). Server side rendering is also an option to have in the later stages of
> any plan. Fan out what lessons we learned from phasekit, list those lessons for me as I may need to pick the most
> important. Investigations of formulations of the CAD engine to take advantage of parallel processing should be in the
> plan. Minumal deps, just ask me if you want to add one. A very important issue is that this CAD engine should be
> accesible to AI AGENTS from the ground up - clear errors, what to do about them, etc. Before we do anything we need a
> NAME - can you suggest some and then create a folder and init it for you and git.

> NOTE i like 'just' tooling like the finance app we are building. Please make sure we include that.
