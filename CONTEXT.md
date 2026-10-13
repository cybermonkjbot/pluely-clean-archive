# Domain glossary

## Interview Event

A macOS Calendar event explicitly identified as an interview by its title,
notes or containing calendar. A generic meeting is not an Interview Event
unless it is placed in an interview calendar or its notes contain interview
metadata.

## Interview Persona

The role, professional identity or candidate profile Joshua is presenting in
an Interview Event. Calendar notes label this value as `as`.

## Target Time Zone

The time zone relevant to the role or interviewer, distinct from the Mac's
local display time. Calendar notes label this value as `timezone`.

## Resume Version

The named résumé variant Joshua intends to use for an Interview Event. It may
be a short name, file path or URL. Calendar notes label this value as `resume`.

## Active Interview Context

The nearest Interview Event that is currently running or begins within the
next 24 hours. It combines the Interview Persona, Target Time Zone, Resume
Version, event time and participants. Missing facts remain missing and are
never inferred as facts.

## Base System Prompt

The user-selected prompt before calendar context is applied. The Active
Interview Context is appended temporarily and does not overwrite it.
