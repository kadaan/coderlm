## Server Status

Current state of the CoderLM server and active session.

**Server:** {{server.status}} | **Projects:** {{server.projects}} | **Sessions:** {{server.active_sessions}}
{% if session %}
**Session:** {{session.session_id}}
**Project:** {{session.project}}
{% endif %}

### Recommended Next Steps

#### 1. View Project Structure
Load the file tree for the indexed project. Use this to orient yourself to the codebase layout before searching for symbols:
```
{{_cli}} structure
```

#### 2. Search for Symbols
Find symbols by name substring. Use this when you know roughly what you're looking for:
```
{{_cli}} search "QUERY"
```
