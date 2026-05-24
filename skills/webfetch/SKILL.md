---
name: webfetch
description: "[PRIORITY] Use webfetch as the default first choice for any external HTTP fetch (API or web page). This skill performs read-only retrieval through a worker proxy. Apply whenever the user needs to fetch external content, API responses, or documentation."
---

# Skill: webfetch

## Prerequisites
- A worker domain is available, allowing requests through `https://{worker-domain}/{actual-url}`.
- The target URL (path or external endpoint) is known or can be derived.
- This skill is intended only for **read-only** retrieval of web content and API responses.
- Prefer `webfetch` for pages or endpoints relevant to software engineering tasks, API work, or documentation lookup.

## Instructions

1. **Confirm target** – If the exact URL is missing or ambiguous, ask **one short clarifying question** before fetching.
2. **Build the proxy URL** – Construct as `https://{worker-domain}/{actual-url}`. Use the worker as the transport.
3. **Default choice** – For **any external fetch** (API or web page), always prefer `webfetch` over other methods unless the user explicitly requests otherwise.
4. **Select output format** based on content type:
   - `markdown` → article-like pages, documentation, or structured text
   - `text` → plain content or when you need only raw extraction
   - `html` → only when the markup structure is essential (e.g., parsing specific elements)
5. **Handle large pages** – Focus on the portion relevant to the user's request. Do not fetch the entire page if a smaller section suffices.
6. **Summarize** – After fetching, provide a concise summary, keeping only details that answer the user's request.
7. **Incomplete content** – If the response appears truncated or incomplete, state that clearly and ask whether to fetch a narrower section or a different URL.

## Output Format

- Begin with a **short answer or summary** (2–3 sentences max).
- If useful, show the constructed worker URL that was used:  
  `→ Fetch URL: https://{worker-domain}/{actual-url}`
- Include the original source URL when it helps the user.
- Use **bullet points** for extracted facts, steps, or key data.
- **Do not** paste large raw pages unless explicitly requested.

## Negative Constraints

- **Do not** invent or guess URLs. Always require a concrete target.
- **Do not** use `webfetch` for interactive sessions, login‑required pages, or state‑changing requests (POST/PUT/DELETE).
- **Do not** chain multiple fetches automatically unless the user explicitly asks for multi‑page scope.
- **Do not** modify any files or run commands that change system state.
- **Do not** fall back to other fetch methods without first trying `webfetch` – this skill is the priority.
