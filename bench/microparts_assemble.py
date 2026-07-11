#!/usr/bin/env python3
"""Assembler for the micro-parts JSON scheme — the grader for the `microparts` generation-benchmark control.

The grammar is deliberately narrow and this file implements exactly it,
nothing more. Each string is parsed ONCE into literal/call/slot nodes by a
quote-aware parser (never regex replace-then-rescan: substituted values are
opaque — argument content must not become code on a later pass).

API: assemble(text) -> (html, None) on success, (None, "code: detail") on
error, where code is one of: bad-json, bad-envelope, duplicate-key,
bad-template, unknown-name, duplicate-arg, unbound-slot, unused-arg,
shadowed-name, depth, cycle.

CLI: python3 microparts_assemble.py doc.json  (HTML to stdout, or the
error to stderr with exit 1).
"""

import json
import re
import sys

NAME_RE = re.compile(r"[a-z_][a-z0-9_]*")
MAX_DEPTH = 8  # active call frames; the first body->part call is depth 1
WS = " \t\r\n"


class MicropartsError(Exception):
    def __init__(self, code, detail):
        super().__init__(f"{code}: {detail}")
        self.code = code


def _pairs_hook(pairs):
    d = {}
    for k, v in pairs:
        if k in d:
            raise MicropartsError("duplicate-key", f"duplicate JSON key {k!r}")
        d[k] = v
    return d


def _parse_envelope(text):
    try:
        doc = json.loads(text, object_pairs_hook=_pairs_hook)
    except MicropartsError:
        raise
    except ValueError as e:
        raise MicropartsError("bad-json", str(e))
    if not isinstance(doc, dict):
        raise MicropartsError("bad-envelope",
                              "top level must be a JSON object")
    unknown = sorted(set(doc) - {"body", "parts"})
    if unknown:
        raise MicropartsError("bad-envelope",
                              f"unknown top-level key(s): {', '.join(unknown)}")
    if "body" not in doc:
        raise MicropartsError("bad-envelope", "missing required key 'body'")
    body = doc["body"]
    if not isinstance(body, str) or not body.strip():
        raise MicropartsError("bad-envelope",
                              "'body' must be a non-empty string")
    parts = doc.get("parts", {})
    if not isinstance(parts, dict):
        raise MicropartsError("bad-envelope", "'parts' must be an object")
    for name, val in parts.items():
        if not NAME_RE.fullmatch(name):
            raise MicropartsError(
                "bad-envelope",
                f"part name {name!r} must match [a-z_][a-z0-9_]* "
                f"(use underscores, not hyphens)")
        if not isinstance(val, str):
            raise MicropartsError("bad-envelope",
                                  f"part {name!r} must be a string")
    return body, parts


def _skip_ws(s, i):
    while i < len(s) and s[i] in WS:
        i += 1
    return i


def _parse_node(s, i, where):
    """Parse one {{...}} node starting just after the '{{' at s[i-2].
    Returns ((name, args_or_None), next_index). args is None for a bare
    node ({{name}} — slot or zero-arg call, resolved later)."""
    i = _skip_ws(s, i)
    m = NAME_RE.match(s, i)
    if not m:
        raise MicropartsError(
            "bad-template",
            f"malformed node after '{{{{' in {where} (names are "
            f"[a-z_][a-z0-9_]* — underscores, not hyphens)")
    name, i = m.group(), m.end()
    i = _skip_ws(s, i)
    if s.startswith("}}", i):
        return (name, None), i + 2
    args = {}
    while True:
        m = NAME_RE.match(s, i)
        if not m:
            raise MicropartsError(
                "bad-template",
                f"expected `key=\"value\"` or '}}}}' in {{{{{name} …}}}} "
                f"in {where}")
        key, i = m.group(), m.end()
        i = _skip_ws(s, i)
        if i >= len(s) or s[i] != "=":
            raise MicropartsError(
                "bad-template", f"expected '=' after {key!r} in "
                f"{{{{{name} …}}}} in {where}")
        i = _skip_ws(s, i + 1)
        if i < len(s) and s[i] == "'":
            raise MicropartsError(
                "bad-template", f"single-quoted value for {key!r} in "
                f"{where} — values must be double-quoted")
        if i >= len(s) or s[i] != '"':
            raise MicropartsError(
                "bad-template", f"expected a double-quoted value for "
                f"{key!r} in {where}")
        i += 1
        val = []
        while True:
            if i >= len(s):
                raise MicropartsError(
                    "bad-template", f"unterminated value for {key!r} in "
                    f"{{{{{name} …}}}} in {where}")
            c = s[i]
            if c == "\\":
                if i + 1 < len(s) and s[i + 1] in '"\\':
                    val.append(s[i + 1])
                    i += 2
                    continue
                raise MicropartsError(
                    "bad-template", f"bad escape in value of {key!r} in "
                    f"{where} — only \\\" and \\\\ exist")
            if c == '"':
                i += 1
                break
            val.append(c)
            i += 1
        if key in args:
            raise MicropartsError(
                "duplicate-arg",
                f"argument {key!r} given twice in {{{{{name} …}}}} in {where}")
        args[key] = "".join(val)
        i = _skip_ws(s, i)
        if s.startswith("}}", i):
            return (name, args), i + 2


def _parse_template(s, where):
    """One string -> a list of ('lit', text) / ('node', name, args) nodes.
    A lone '{' or '}' (and a '}}' outside a node) is literal text; only
    '{{' opens a node, and there is no escape for a literal '{{'."""
    nodes, i = [], 0
    while i < len(s):
        j = s.find("{{", i)
        if j < 0:
            nodes.append(("lit", s[i:]))
            break
        if j > i:
            nodes.append(("lit", s[i:j]))
        (name, args), i = _parse_node(s, j + 2, where)
        nodes.append(("node", name, args))
    return nodes


def assemble(text):
    """Assemble a micro-parts JSON document into HTML.
    Returns (html, None) or (None, "code: detail")."""
    try:
        return _assemble(text), None
    except MicropartsError as e:
        return None, str(e)


def _assemble(text):
    body, parts = _parse_envelope(text)
    part_nodes = {name: _parse_template(src, f"part {name!r}")
                  for name, src in parts.items()}
    body_nodes = _parse_template(body, "body")
    # A part's slot set is static: the bare names in its template that are
    # not part names (bare part names are zero-arg calls).
    slots = {name: {n[1] for n in nodes
                    if n[0] == "node" and n[2] is None and n[1] not in parts}
             for name, nodes in part_nodes.items()}

    def expand(nodes, bindings, stack):
        # bindings is None in body context; inside a part it maps the
        # call's args. Substituted values are OPAQUE: appended as literal
        # text, never re-scanned — only nodes parsed from the original
        # templates recurse.
        out = []
        for node in nodes:
            if node[0] == "lit":
                out.append(node[1])
                continue
            name, args = node[1], node[2]
            if args is None and bindings is not None and name not in parts:
                out.append(bindings[name])  # slot — bound, checked by caller
                continue
            if name not in parts:
                where = f"part {stack[-1]!r}" if stack else "body"
                raise MicropartsError(
                    "unknown-name", f"{{{{{name}}}}} in {where} is not a "
                    f"part" + ("" if stack else
                               " (slots only exist inside parts)"))
            call_args = args or {}
            shadow = sorted(set(call_args) & set(parts))
            if shadow:
                raise MicropartsError(
                    "shadowed-name",
                    f"argument(s) {', '.join(shadow)} in a call to "
                    f"{name!r} shadow part names")
            if name in stack:
                chain = " -> ".join(stack + [name])
                raise MicropartsError("cycle", f"part call cycle: {chain}")
            if len(stack) + 1 > MAX_DEPTH:
                chain = " -> ".join(stack + [name])
                raise MicropartsError(
                    "depth", f"part calls nested deeper than {MAX_DEPTH}: "
                    f"{chain}")
            missing = sorted(slots[name] - set(call_args))
            if missing:
                raise MicropartsError(
                    "unbound-slot", f"call to {name!r} does not bind "
                    f"slot(s): {', '.join(missing)}")
            extra = sorted(set(call_args) - slots[name])
            if extra:
                raise MicropartsError(
                    "unused-arg", f"call to {name!r} passes argument(s) "
                    f"with no matching slot: {', '.join(extra)}")
            out.append(expand(part_nodes[name], call_args, stack + [name]))
        return "".join(out)

    return expand(body_nodes, None, [])


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: microparts_assemble.py doc.json")
    with open(sys.argv[1]) as fh:
        html, err = assemble(fh.read())
    if err:
        sys.exit(f"error: {err}")
    sys.stdout.write(html)
