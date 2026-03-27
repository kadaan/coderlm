#!/usr/bin/env python3
"""CLI wrapper for the coderlm-server API.

Manages session state and provides clean commands for codebase exploration.
All state is cached in .claude/coderlm_state/session.json relative to cwd.

Usage:
  python3 coderlm_cli.py init [--port PORT] [--cwd PATH]
  python3 coderlm_cli.py structure [--depth N]
  python3 coderlm_cli.py symbols [--kind KIND] [--file FILE] [--limit N] [--branch base]
  python3 coderlm_cli.py search QUERY [--limit N]
  python3 coderlm_cli.py impl SYMBOL --file FILE [--branch base]
  python3 coderlm_cli.py callers SYMBOL --file FILE [--limit N] [--branch base]
  python3 coderlm_cli.py tests SYMBOL --file FILE [--limit N] [--branch base]
  python3 coderlm_cli.py variables FUNCTION --file FILE
  python3 coderlm_cli.py peek FILE [--start N] [--end N]
  python3 coderlm_cli.py grep PATTERN [--max-matches N] [--context-lines N] [--scope all|code]
  python3 coderlm_cli.py chunks FILE [--size N] [--overlap N]
  python3 coderlm_cli.py define-file FILE DEFINITION
  python3 coderlm_cli.py redefine-file FILE DEFINITION
  python3 coderlm_cli.py define-symbol SYMBOL --file FILE DEFINITION
  python3 coderlm_cli.py redefine-symbol SYMBOL --file FILE DEFINITION
  python3 coderlm_cli.py mark FILE TYPE
  python3 coderlm_cli.py save-annotations
  python3 coderlm_cli.py load-annotations
  python3 coderlm_cli.py history [--limit N]
  python3 coderlm_cli.py status
  python3 coderlm_cli.py cleanup

Review commands (require an active session):
  python3 coderlm_cli.py review-init --base REF [--head REF]
  python3 coderlm_cli.py review-status
  python3 coderlm_cli.py review-cleanup
  python3 coderlm_cli.py review-summary
  python3 coderlm_cli.py review-files
  python3 coderlm_cli.py review-symbols [--change added|deleted|modified] [--file PATH] [--kind KIND]
  python3 coderlm_cli.py review-file-diff --file PATH
  python3 coderlm_cli.py review-safety
  python3 coderlm_cli.py review-impact SYMBOL --file PATH
  python3 coderlm_cli.py review-test-coverage
  python3 coderlm_cli.py review-update [--head REF]
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

STATE_DIR = Path(os.environ.get("CODERLM_STATE_DIR", ".claude/coderlm_state"))
STATE_FILE = STATE_DIR / "session.json"


def _load_state() -> dict:
    if not STATE_FILE.exists():
        return {}
    with STATE_FILE.open() as f:
        return json.load(f)


def _save_state(state: dict) -> None:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    with STATE_FILE.open("w") as f:
        json.dump(state, f, indent=2)


def _clear_state() -> None:
    if STATE_FILE.exists():
        STATE_FILE.unlink()


def _base_url(state: dict) -> str:
    host = state.get("host", "127.0.0.1")
    port = state.get("port", 3000)
    return f"http://{host}:{port}/api/v1"


def _session_id(state: dict) -> str:
    sid = state.get("session_id")
    if not sid:
        print("ERROR: No active session. Run: coderlm_cli.py init", file=sys.stderr)
        sys.exit(1)
    return sid


def _request(
    method: str,
    url: str,
    data: dict | None = None,
    headers: dict | None = None,
    timeout: int = 30,
) -> dict:
    hdrs = headers or {}
    body = None
    if data is not None:
        body = json.dumps(data).encode("utf-8")
        hdrs["Content-Type"] = "application/json"

    req = urllib.request.Request(url, data=body, headers=hdrs, method=method)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            raw = resp.read().decode("utf-8")
            return json.loads(raw) if raw.strip() else {}
    except urllib.error.HTTPError as e:
        body_text = e.read().decode("utf-8", errors="replace")
        try:
            err = json.loads(body_text)
        except json.JSONDecodeError:
            err = {"error": body_text, "status": e.code}

        if e.code == 410:
            print(
                "ERROR: Project was evicted from server. Run: coderlm_cli.py init",
                file=sys.stderr,
            )
            _clear_state()
            sys.exit(1)

        print(json.dumps(err, indent=2))
        sys.exit(1)
    except urllib.error.URLError as e:
        print(
            f"ERROR: Cannot connect to coderlm-server: {e.reason}\n"
            f"Make sure the server is running: coderlm-server serve",
            file=sys.stderr,
        )
        sys.exit(1)


def _get(state: dict, path: str, params: dict | None = None) -> dict:
    base = _base_url(state)
    url = f"{base}{path}"
    if params:
        clean = {k: v for k, v in params.items() if v is not None}
        if clean:
            url += "?" + urllib.parse.urlencode(clean)
    return _request("GET", url, headers={"X-Session-Id": _session_id(state)})


def _post(state: dict, path: str, data: dict) -> dict:
    base = _base_url(state)
    url = f"{base}{path}"
    return _request("POST", url, data=data, headers={"X-Session-Id": _session_id(state)})


def _delete(state: dict, path: str) -> dict:
    base = _base_url(state)
    url = f"{base}{path}"
    return _request("DELETE", url, headers={"X-Session-Id": _session_id(state)})


def _get_raw(state: dict, path: str, params: dict | None = None) -> dict:
    """GET without requiring a session (for admin/review lifecycle endpoints)."""
    base = _base_url(state)
    url = f"{base}{path}"
    if params:
        clean = {k: v for k, v in params.items() if v is not None}
        if clean:
            url += "?" + urllib.parse.urlencode(clean)
    return _request("GET", url)


def _post_raw(state: dict, path: str, data: dict) -> dict:
    """POST without requiring a session (for review creation)."""
    base = _base_url(state)
    url = f"{base}{path}"
    return _request("POST", url, data=data)


def _delete_raw(state: dict, path: str) -> dict:
    """DELETE without requiring a session."""
    base = _base_url(state)
    url = f"{base}{path}"
    return _request("DELETE", url)


def _output(result: dict) -> None:
    print(json.dumps(result, indent=2))


# ── Commands ──────────────────────────────────────────────────────────


def cmd_init(args: argparse.Namespace) -> None:
    cwd = os.path.abspath(args.cwd or os.getcwd())
    host = args.host or "127.0.0.1"
    port = args.port or 3000
    base = f"http://{host}:{port}/api/v1"

    # Check server health first
    try:
        health = _request("GET", f"{base}/health")
    except SystemExit:
        return

    # Create session
    result = _request("POST", f"{base}/sessions", data={"cwd": cwd})
    state = {
        "session_id": result["session_id"],
        "host": host,
        "port": port,
        "project": cwd,
        "created_at": result.get("created_at", ""),
    }
    _save_state(state)

    print(f"Session created: {result['session_id']}")
    print(f"Project: {cwd}")
    print(f"Server: {health.get('status', 'ok')} "
          f"({health.get('projects', 0)} projects, "
          f"{health.get('active_sessions', 0)} sessions)")


def cmd_status(args: argparse.Namespace) -> None:
    state = _load_state()
    if not state:
        # No session — just check server health
        host = args.host or "127.0.0.1"
        port = args.port or 3000
        base = f"http://{host}:{port}/api/v1"
        result = _request("GET", f"{base}/health")
        _output(result)
        return

    base = _base_url(state)
    health = _request("GET", f"{base}/health")
    info = {"server": health, "session": state}

    # Try to get session details
    sid = state.get("session_id")
    if sid:
        try:
            session_info = _request(
                "GET",
                f"{base}/sessions/{sid}",
            )
            info["session_details"] = session_info
        except SystemExit:
            info["session_details"] = "session may have expired"

    _output(info)


def cmd_structure(args: argparse.Namespace) -> None:
    state = _load_state()
    params = {}
    if args.depth is not None:
        params["depth"] = args.depth
    _output(_get(state, "/structure", params))


def cmd_symbols(args: argparse.Namespace) -> None:
    state = _load_state()
    params = {}
    if args.kind:
        params["kind"] = args.kind
    if args.file:
        params["file"] = args.file
    if args.limit is not None:
        params["limit"] = args.limit
    if getattr(args, "branch", None):
        params["branch"] = args.branch
    _output(_get(state, "/symbols", params))


def cmd_search(args: argparse.Namespace) -> None:
    state = _load_state()
    params = {"q": args.query}
    if args.limit is not None:
        params["limit"] = args.limit
    _output(_get(state, "/symbols/search", params))


def cmd_impl(args: argparse.Namespace) -> None:
    state = _load_state()
    params: dict = {"symbol": args.symbol, "file": args.file}
    if getattr(args, "branch", None):
        params["branch"] = args.branch
    _output(_get(state, "/symbols/implementation", params))


def cmd_callers(args: argparse.Namespace) -> None:
    state = _load_state()
    params: dict = {"symbol": args.symbol, "file": args.file}
    if args.limit is not None:
        params["limit"] = args.limit
    if getattr(args, "branch", None):
        params["branch"] = args.branch
    _output(_get(state, "/symbols/callers", params))


def cmd_tests(args: argparse.Namespace) -> None:
    state = _load_state()
    params: dict = {"symbol": args.symbol, "file": args.file}
    if args.limit is not None:
        params["limit"] = args.limit
    if getattr(args, "branch", None):
        params["branch"] = args.branch
    _output(_get(state, "/symbols/tests", params))


def cmd_variables(args: argparse.Namespace) -> None:
    state = _load_state()
    params = {"function": args.function, "file": args.file}
    _output(_get(state, "/symbols/variables", params))


def cmd_peek(args: argparse.Namespace) -> None:
    state = _load_state()
    params = {"file": args.file}
    if args.start is not None:
        params["start"] = args.start
    if args.end is not None:
        params["end"] = args.end
    _output(_get(state, "/peek", params))


def cmd_grep(args: argparse.Namespace) -> None:
    state = _load_state()
    params = {"pattern": args.pattern}
    if args.max_matches is not None:
        params["max_matches"] = args.max_matches
    if args.context_lines is not None:
        params["context_lines"] = args.context_lines
    if args.scope is not None:
        params["scope"] = args.scope
    _output(_get(state, "/grep", params))


def cmd_chunks(args: argparse.Namespace) -> None:
    state = _load_state()
    params = {"file": args.file}
    if args.size is not None:
        params["size"] = args.size
    if args.overlap is not None:
        params["overlap"] = args.overlap
    _output(_get(state, "/chunk_indices", params))


def cmd_define_file(args: argparse.Namespace) -> None:
    state = _load_state()
    _output(_post(state, "/structure/define", {
        "file": args.file,
        "definition": args.definition,
    }))


def cmd_redefine_file(args: argparse.Namespace) -> None:
    state = _load_state()
    _output(_post(state, "/structure/redefine", {
        "file": args.file,
        "definition": args.definition,
    }))


def cmd_define_symbol(args: argparse.Namespace) -> None:
    state = _load_state()
    _output(_post(state, "/symbols/define", {
        "symbol": args.symbol,
        "file": args.file,
        "definition": args.definition,
    }))


def cmd_redefine_symbol(args: argparse.Namespace) -> None:
    state = _load_state()
    _output(_post(state, "/symbols/redefine", {
        "symbol": args.symbol,
        "file": args.file,
        "definition": args.definition,
    }))


def cmd_mark(args: argparse.Namespace) -> None:
    state = _load_state()
    _output(_post(state, "/structure/mark", {
        "file": args.file,
        "mark": args.type,
    }))


def cmd_history(args: argparse.Namespace) -> None:
    state = _load_state()
    params = {}
    if args.limit is not None:
        params["limit"] = args.limit
    _output(_get(state, "/history", params))


def cmd_save_annotations(args: argparse.Namespace) -> None:
    state = _load_state()
    _output(_post(state, "/annotations/save", {}))


def cmd_load_annotations(args: argparse.Namespace) -> None:
    state = _load_state()
    _output(_post(state, "/annotations/load", {}))


def cmd_cleanup(args: argparse.Namespace) -> None:
    state = _load_state()
    if not state.get("session_id"):
        print("No active session.")
        return

    # Clean up review first if one is attached
    review_id = state.get("review_id")
    if review_id:
        try:
            _delete_raw(state, f"/reviews/{review_id}")
        except SystemExit:
            pass  # ignore errors during cleanup

    base = _base_url(state)
    sid = state["session_id"]
    _request("DELETE", f"{base}/sessions/{sid}")
    _clear_state()
    print(f"Session {sid} deleted.")


# ── Review commands ────────────────────────────────────────────────────


def cmd_review_init(args: argparse.Namespace) -> None:
    """Create a review, index the base branch, and attach it to the current session."""
    import time

    state = _load_state()
    if not state.get("session_id"):
        print("ERROR: No active session. Run: coderlm_cli.py init", file=sys.stderr)
        sys.exit(1)

    cwd = state.get("project", os.getcwd())
    body: dict = {"cwd": cwd, "base_ref": args.base}
    if args.head:
        body["head_ref"] = args.head

    print(f"Creating review: {args.base}..{args.head or 'HEAD'}")
    result = _post_raw(state, "/reviews", body)
    review_id = result["review_id"]
    print(f"Review created: {review_id}")
    print(f"Status: {result.get('status', 'unknown')}")

    # Attach the review to the current session
    _post(state, f"/reviews/{review_id}/attach", {})
    state["review_id"] = review_id
    _save_state(state)

    print(f"Attached to session. Polling for ready...")

    # Poll until ready (up to 60 seconds)
    for _ in range(120):
        time.sleep(0.5)
        status_result = _get_raw(state, f"/reviews/{review_id}")
        status = status_result.get("status", "unknown")
        if status == "ready":
            print(f"Review ready.")
            if status_result.get("stats"):
                s = status_result["stats"]
                print(f"  Files: +{s.get('files_added',0)} ~{s.get('files_modified',0)} -{s.get('files_deleted',0)}")
                print(f"  Symbols: +{s.get('symbols_added',0)} ~{s.get('symbols_modified',0)} -{s.get('symbols_deleted',0)}")
            return
        if status == "error":
            print(f"ERROR: Review failed.", file=sys.stderr)
            sys.exit(1)

    print("WARNING: Review indexing timed out. Try 'review-status' to check progress.")


def cmd_review_status(args: argparse.Namespace) -> None:
    state = _load_state()
    review_id = state.get("review_id")
    if not review_id:
        print("No review attached to current session.")
        return
    _output(_get_raw(state, f"/reviews/{review_id}"))


def cmd_review_cleanup(args: argparse.Namespace) -> None:
    state = _load_state()
    review_id = state.get("review_id")
    if not review_id:
        print("No review attached.")
        return
    _delete_raw(state, f"/reviews/{review_id}")
    del state["review_id"]
    _save_state(state)
    print(f"Review {review_id} deleted and worktree cleaned up.")


def cmd_review_summary(args: argparse.Namespace) -> None:
    state = _load_state()
    _output(_get(state, "/review/summary"))


def cmd_review_files(args: argparse.Namespace) -> None:
    state = _load_state()
    _output(_get(state, "/review/files"))


def cmd_review_symbols(args: argparse.Namespace) -> None:
    state = _load_state()
    params: dict = {}
    if getattr(args, "change", None):
        params["change"] = args.change
    if getattr(args, "file", None):
        params["file"] = args.file
    if getattr(args, "kind", None):
        params["kind"] = args.kind
    _output(_get(state, "/review/symbols", params))


def cmd_review_file_diff(args: argparse.Namespace) -> None:
    state = _load_state()
    _output(_get(state, "/review/file-diff", {"file": args.file}))


def cmd_review_safety(args: argparse.Namespace) -> None:
    state = _load_state()
    params: dict = {}
    if getattr(args, "depth", None) is not None:
        params["depth"] = args.depth
    _output(_get(state, "/review/safety", params))


def cmd_review_impact(args: argparse.Namespace) -> None:
    state = _load_state()
    params: dict = {"symbol": args.symbol, "file": args.file}
    if getattr(args, "depth", None) is not None:
        params["depth"] = args.depth
    _output(_get(state, "/review/impact", params))


def cmd_review_test_coverage(args: argparse.Namespace) -> None:
    state = _load_state()
    _output(_get(state, "/review/test-coverage"))


def cmd_review_update(args: argparse.Namespace) -> None:
    """Incrementally update the attached review to a new head commit."""
    state = _load_state()
    review_id = state.get("review_id")
    if not review_id:
        print("ERROR: No review attached. Run: coderlm_cli.py review-init --base REF", file=sys.stderr)
        sys.exit(1)

    body: dict = {}
    if getattr(args, "head", None):
        body["head_ref"] = args.head

    result = _post_raw(state, f"/reviews/{review_id}/update", body)
    _output(result)


# ── Parser ────────────────────────────────────────────────────────────


def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        prog="coderlm_cli",
        description="CLI wrapper for coderlm-server API",
    )

    sub = p.add_subparsers(dest="cmd", required=True)

    # init
    p_init = sub.add_parser("init", help="Create a session for the current project")
    p_init.add_argument("--cwd", help="Project directory (default: $PWD)")
    p_init.add_argument("--host", default=None, help="Server host (default: 127.0.0.1)")
    p_init.add_argument("--port", type=int, default=None, help="Server port (default: 3000)")
    p_init.set_defaults(func=cmd_init)

    # status
    p_status = sub.add_parser("status", help="Show server and session status")
    p_status.add_argument("--host", default=None)
    p_status.add_argument("--port", type=int, default=None)
    p_status.set_defaults(func=cmd_status)

    # structure
    p_struct = sub.add_parser("structure", help="Get project file tree")
    p_struct.add_argument("--depth", type=int, default=None, help="Tree depth (0=unlimited)")
    p_struct.set_defaults(func=cmd_structure)

    # symbols
    p_sym = sub.add_parser("symbols", help="List symbols")
    p_sym.add_argument("--kind", help="Filter: function, method, class, struct, enum, trait, interface, constant, type, module")
    p_sym.add_argument("--file", help="Filter by file path")
    p_sym.add_argument("--limit", type=int, default=None)
    p_sym.add_argument("--branch", choices=["base"], default=None, help="Query base branch (requires attached review)")
    p_sym.set_defaults(func=cmd_symbols)

    # search
    p_search = sub.add_parser("search", help="Search symbols by name")
    p_search.add_argument("query", help="Search term")
    p_search.add_argument("--limit", type=int, default=None)
    p_search.set_defaults(func=cmd_search)

    # impl
    p_impl = sub.add_parser("impl", help="Get full source of a symbol")
    p_impl.add_argument("symbol", help="Symbol name")
    p_impl.add_argument("--file", required=True, help="File containing the symbol")
    p_impl.add_argument("--branch", choices=["base"], default=None, help="Read from base branch (requires attached review)")
    p_impl.set_defaults(func=cmd_impl)

    # callers
    p_callers = sub.add_parser("callers", help="Find call sites for a symbol")
    p_callers.add_argument("symbol", help="Symbol name")
    p_callers.add_argument("--file", required=True, help="File containing the symbol")
    p_callers.add_argument("--limit", type=int, default=None)
    p_callers.add_argument("--branch", choices=["base"], default=None, help="Query base branch (requires attached review)")
    p_callers.set_defaults(func=cmd_callers)

    # tests
    p_tests = sub.add_parser("tests", help="Find tests referencing a symbol")
    p_tests.add_argument("symbol", help="Symbol name")
    p_tests.add_argument("--file", required=True, help="File containing the symbol")
    p_tests.add_argument("--limit", type=int, default=None)
    p_tests.add_argument("--branch", choices=["base"], default=None, help="Query base branch (requires attached review)")
    p_tests.set_defaults(func=cmd_tests)

    # variables
    p_vars = sub.add_parser("variables", help="List local variables in a function")
    p_vars.add_argument("function", help="Function name")
    p_vars.add_argument("--file", required=True, help="File containing the function")
    p_vars.set_defaults(func=cmd_variables)

    # peek
    p_peek = sub.add_parser("peek", help="Read a line range from a file")
    p_peek.add_argument("file", help="File path")
    p_peek.add_argument("--start", type=int, default=None, help="Start line (0-indexed)")
    p_peek.add_argument("--end", type=int, default=None, help="End line (exclusive)")
    p_peek.set_defaults(func=cmd_peek)

    # grep
    p_grep = sub.add_parser("grep", help="Regex search across all files")
    p_grep.add_argument("pattern", help="Regex pattern")
    p_grep.add_argument("--max-matches", type=int, default=None)
    p_grep.add_argument("--context-lines", type=int, default=None)
    p_grep.add_argument("--scope", choices=["all", "code"], default=None,
                         help="Scope filter: 'all' (default) or 'code' (skip comments/strings)")
    p_grep.set_defaults(func=cmd_grep)

    # chunks
    p_chunks = sub.add_parser("chunks", help="Compute chunk boundaries for a file")
    p_chunks.add_argument("file", help="File path")
    p_chunks.add_argument("--size", type=int, default=None, help="Chunk size in bytes")
    p_chunks.add_argument("--overlap", type=int, default=None, help="Overlap between chunks")
    p_chunks.set_defaults(func=cmd_chunks)

    # define-file
    p_dfile = sub.add_parser("define-file", help="Set a description for a file")
    p_dfile.add_argument("file", help="File path")
    p_dfile.add_argument("definition", help="Human-readable description")
    p_dfile.set_defaults(func=cmd_define_file)

    # redefine-file
    p_rdfile = sub.add_parser("redefine-file", help="Update a file description")
    p_rdfile.add_argument("file", help="File path")
    p_rdfile.add_argument("definition", help="Updated description")
    p_rdfile.set_defaults(func=cmd_redefine_file)

    # define-symbol
    p_dsym = sub.add_parser("define-symbol", help="Set a description for a symbol")
    p_dsym.add_argument("symbol", help="Symbol name")
    p_dsym.add_argument("--file", required=True, help="File containing the symbol")
    p_dsym.add_argument("definition", help="Human-readable description")
    p_dsym.set_defaults(func=cmd_define_symbol)

    # redefine-symbol
    p_rdsym = sub.add_parser("redefine-symbol", help="Update a symbol description")
    p_rdsym.add_argument("symbol", help="Symbol name")
    p_rdsym.add_argument("--file", required=True, help="File containing the symbol")
    p_rdsym.add_argument("definition", help="Updated description")
    p_rdsym.set_defaults(func=cmd_redefine_symbol)

    # mark
    p_mark = sub.add_parser("mark", help="Tag a file with a category")
    p_mark.add_argument("file", help="File path")
    p_mark.add_argument("type", choices=["documentation", "ignore", "test", "config", "generated", "custom"],
                         help="Mark type")
    p_mark.set_defaults(func=cmd_mark)

    # history
    p_hist = sub.add_parser("history", help="Session command history")
    p_hist.add_argument("--limit", type=int, default=None)
    p_hist.set_defaults(func=cmd_history)

    # save-annotations
    p_save = sub.add_parser("save-annotations", help="Save annotations to disk (.coderlm/annotations.json)")
    p_save.set_defaults(func=cmd_save_annotations)

    # load-annotations
    p_load = sub.add_parser("load-annotations", help="Load annotations from disk")
    p_load.set_defaults(func=cmd_load_annotations)

    # cleanup
    p_clean = sub.add_parser("cleanup", help="Delete the current session")
    p_clean.set_defaults(func=cmd_cleanup)

    # ── Review commands ──────────────────────────────────────────────

    # review-init
    p_ri = sub.add_parser("review-init", help="Create a review and attach it to this session")
    p_ri.add_argument("--base", required=True, help="Base branch/ref (e.g. main, origin/main)")
    p_ri.add_argument("--head", default=None, help="Head ref (default: HEAD)")
    p_ri.set_defaults(func=cmd_review_init)

    # review-status
    p_rs = sub.add_parser("review-status", help="Show status of the attached review")
    p_rs.set_defaults(func=cmd_review_status)

    # review-cleanup
    p_rc = sub.add_parser("review-cleanup", help="Delete the attached review and clean up its worktree")
    p_rc.set_defaults(func=cmd_review_cleanup)

    # review-summary
    p_rsum = sub.add_parser("review-summary", help="High-level diff stats")
    p_rsum.set_defaults(func=cmd_review_summary)

    # review-files
    p_rf = sub.add_parser("review-files", help="List changed files")
    p_rf.set_defaults(func=cmd_review_files)

    # review-symbols
    p_rsym = sub.add_parser("review-symbols", help="List changed symbols")
    p_rsym.add_argument("--change", choices=["added", "deleted", "modified"], default=None)
    p_rsym.add_argument("--file", default=None, help="Filter by file path")
    p_rsym.add_argument("--kind", default=None, help="Filter by symbol kind")
    p_rsym.set_defaults(func=cmd_review_symbols)

    # review-file-diff
    p_rfd = sub.add_parser("review-file-diff", help="Unified diff for a specific file")
    p_rfd.add_argument("--file", required=True, help="File path")
    p_rfd.set_defaults(func=cmd_review_file_diff)

    # review-safety
    p_rsa = sub.add_parser("review-safety", help="Find deleted/changed symbols with unmodified callers (breakage risk)")
    p_rsa.add_argument("--depth", type=int, default=None,
                        help="Transitive caller depth (1=direct only, up to 3; default: 1)")
    p_rsa.set_defaults(func=cmd_review_safety)

    # review-impact
    p_rimp = sub.add_parser("review-impact", help="Blast radius for a specific changed symbol")
    p_rimp.add_argument("symbol", help="Symbol name")
    p_rimp.add_argument("--file", required=True, help="File containing the symbol")
    p_rimp.add_argument("--depth", type=int, default=None,
                         help="Transitive caller depth (1=direct only, up to 3; default: 1)")
    p_rimp.set_defaults(func=cmd_review_impact)

    # review-test-coverage
    p_rtc = sub.add_parser("review-test-coverage", help="Test coverage of added/modified symbols")
    p_rtc.set_defaults(func=cmd_review_test_coverage)

    # review-update
    p_ru = sub.add_parser("review-update", help="Incrementally update the review to a new head commit")
    p_ru.add_argument("--head", default=None, help="New head ref (default: HEAD)")
    p_ru.set_defaults(func=cmd_review_update)

    return p


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
