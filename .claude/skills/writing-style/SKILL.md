---
name: writing-style
description: Reviews and writes text for clarity, tone, and consistency. Use when writing UI copy, documentation, handbook content, or error messages.
---

# Writing Style

STARTER_CHARACTER = ✍️

Review and write text that's clear, helpful, and human.

## Adapting to Project Context

Check for existing voice guidelines (docs/handbook, CLAUDE.md, style guides). When found, follow them. When none exist, use the defaults below.

## Default Voice

**Tone**: Friendly and direct. Like a helpful colleague, not a corporate manual.

**Qualities**:
- **Clear**: Say what you mean. No jargon unless the audience expects it.
- **Concise**: Every word earns its place. Cut filler.
- **Human**: Write how people talk. Contractions are fine.
- **Helpful**: Guide toward action, not just describe state.

## Punctuation

Avoid dashes and em-dashes. Use colons, periods, or restructure the sentence instead.

Anti-pattern: "Couldn't save — you're offline"

## Content Categories

### Buttons & Actions

Use action verbs. State what happens, not what the element is.

Anti-patterns:
- "Submit": vague
- "OK": doesn't describe action
- "Click here": describes mechanics, not outcome
- "Process": passive, corporate

Destructive actions: be explicit about what will be destroyed.

### Error Messages

Help users fix the problem. Structure: what happened, then what to do.

Anti-patterns:
- "Error 500": meaningless to users
- "Invalid input": which input? why?
- "Something went wrong": no guidance
- Blaming language ("You failed to...")

### Empty States

Empty isn't an error. It's an opportunity to guide action.

Anti-patterns:
- "No results": dead end
- "Nothing here": states obvious, doesn't help
- Blank screen with no guidance

### Labels & Headings

Be specific. Generic labels force users to guess.

Anti-patterns:
- "Settings" (for what?)
- "Details" (which details?)
- "Manage" (manage what?)

### Tooltips & Help Text

Explain the non-obvious. Don't repeat what's already visible.

Anti-patterns:
- Tooltip on "Save" button: "Click to save" (useless)
- Help text restating the label
- Walls of text

Tooltips should explain why, when, or edge cases.

### Confirmation Dialogs

State the consequence clearly. Make the action button match the question.

Anti-patterns:
- "Are you sure?" + "Yes/No": yes to what?
- Vague consequences ("This action cannot be undone")
- Action buttons that don't match the question asked

### Documentation & Handbook

Same principles apply: clear, concise, human.

Additional considerations:
- Lead with what the reader needs to do, not background
- Use headings that answer questions ("How do I..." not "About the...")
- Keep paragraphs short. Scannable over walls of text.
- Code examples over lengthy explanations when applicable

## Terminology Consistency

Pick one term and stick with it. Document choices if the project doesn't have a glossary.

Anti-patterns:
- "Map" in one place, "guide" in another for the same concept
- "Remove" vs "delete" used interchangeably
- "User" vs "member" vs "account"

A mini-glossary keeps terms consistent:
```
place (not: location, spot, point)
adventure map (not: guide, collection)
curator (not: editor, creator, admin)
```

## Accessibility

- Avoid directional references ("click the button on the left")
- Don't rely on color alone ("fields marked in red")
- Keep reading level accessible (aim for grade 8 / age 14)
- Labels should make sense out of context (for screen readers)

## Localization Readiness

Avoid patterns that break in translation:

- Don't concatenate strings ("You have " + n + " items"). Use templates with placeholders.
- Avoid idioms ("piece of cake", "ballpark figure")
- Leave room for text expansion (German/French often 30% longer)
- Don't embed formatting in copy ("Click **here**"). Emphasis may not translate.

## Review Checklist

When reviewing copy in a feature or document:

- [ ] **Scan for generic text**: "Submit", "Error", "Settings", "Details"
- [ ] **Check error paths**: Do errors explain what went wrong and what to do?
- [ ] **Check empty states**: Do they guide toward action?
- [ ] **Verify button labels**: Do they describe the outcome?
- [ ] **Consistency check**: Same concept = same word throughout
- [ ] **Read aloud**: Does it sound human or robotic?
- [ ] **Check length**: Fits UI constraints? Scannable?
- [ ] **Accessibility**: Makes sense without visual context?
- [ ] **No dashes**: Colons or periods instead of em-dashes
