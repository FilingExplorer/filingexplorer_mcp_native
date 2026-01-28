#!/usr/bin/env python3
"""
API Coverage Diff Tool

Compares the FilingExplorer API swagger spec against the MCP server implementation
to identify missing routes and coverage gaps.

Usage:
    python scripts/api_coverage_diff.py
"""

import re
import yaml
from pathlib import Path
from dataclasses import dataclass
from typing import Optional


@dataclass
class SwaggerRoute:
    path: str
    method: str
    summary: str
    tag: str

    @property
    def normalized_path(self) -> str:
        """Convert /v1/companies/{cik}/calendar to companies/{cik}/calendar"""
        return self.path.replace("/v1/", "")


@dataclass
class McpTool:
    name: str
    endpoint: str
    method: str


def parse_swagger(swagger_path: Path) -> list[SwaggerRoute]:
    """Parse swagger.yaml and extract all routes."""
    with open(swagger_path) as f:
        spec = yaml.safe_load(f)

    routes = []
    for path, methods in spec.get("paths", {}).items():
        for method, details in methods.items():
            if method in ("get", "post", "put", "patch", "delete"):
                tags = details.get("tags", ["Unknown"])
                routes.append(SwaggerRoute(
                    path=path,
                    method=method.upper(),
                    summary=details.get("summary", ""),
                    tag=tags[0] if tags else "Unknown"
                ))

    return routes


def parse_mcp_implementation(main_rs_path: Path) -> tuple[set[str], list[McpTool]]:
    """
    Parse main.rs and extract:
    1. Set of tool names from execute_actual_tool match block
    2. List of tools with their endpoint mappings
    """
    content = main_rs_path.read_text()

    tool_names = set()

    # Extract tool names from execute_tool match arms (for special tools like "search")
    tool_match = re.search(
        r'async fn execute_tool.*?match name \{(.*?)\n\s+_ =>',
        content,
        re.DOTALL
    )
    if tool_match:
        match_block = tool_match.group(1)
        # Find all "tool_name" => patterns, but exclude meta-tools
        names = set(re.findall(r'"(\w+)"', match_block))
        meta_tools = {"list_tool_categories", "search_tools", "execute_tool"}
        tool_names.update(names - meta_tools)

    # Extract tool names from execute_actual_tool match arms
    tool_match = re.search(
        r'async fn execute_actual_tool.*?match name \{(.*?)\n\s+_ =>',
        content,
        re.DOTALL
    )

    if tool_match:
        match_block = tool_match.group(1)
        # Find all "tool_name" => patterns
        tool_names.update(re.findall(r'"(\w+)"', match_block))

    tools = []

    # For each tool, find its function and extract endpoint info
    for tool_name in tool_names:
        # Find the function - look for async fn tool_name with various endings
        # Use a more permissive pattern
        patterns = [
            rf'async fn {tool_name}\(&self[^{{]*\{{(.*?)\n    \}}\n',
            rf'async fn {tool_name}\(&self[^{{]*\{{(.*?)\n        Ok\(',
        ]

        func_body = None
        for pattern in patterns:
            func_match = re.search(pattern, content, re.DOTALL)
            if func_match:
                func_body = func_match.group(1)
                break

        if not func_body:
            # Try a broader search
            start = content.find(f"async fn {tool_name}(")
            if start != -1:
                # Find the matching closing brace
                end = content.find("\n    async fn", start + 1)
                if end == -1:
                    end = content.find("\n}\n", start)
                if end != -1:
                    func_body = content[start:end]

        if func_body:
            endpoint = None
            method = "GET"  # default

            # Check for format! endpoint
            fmt_match = re.search(r'let endpoint = format!\("([^"]+)"', func_body)
            if fmt_match:
                endpoint = fmt_match.group(1)

            # Check for client.get/post/etc with direct string
            for http_method, pattern in [
                ("GET", r'client\.get\("([^"]+)"'),
                ("POST", r'client\.post\("([^"]+)"'),
                ("PATCH", r'client\.patch\("([^"]+)"'),
                ("DELETE", r'client\.delete\("([^"]+)"'),
            ]:
                match = re.search(pattern, func_body)
                if match:
                    if endpoint is None:
                        endpoint = match.group(1)
                    method = http_method
                    break

            # Also check for client.method(&endpoint, ...)
            for http_method, pattern in [
                ("GET", r'client\.get\(&endpoint'),
                ("POST", r'client\.post\(&endpoint'),
                ("PATCH", r'client\.patch\(&endpoint'),
                ("DELETE", r'client\.delete\(&endpoint'),
            ]:
                if re.search(pattern, func_body):
                    method = http_method
                    break

            if endpoint:
                tools.append(McpTool(
                    name=tool_name,
                    endpoint=endpoint,
                    method=method
                ))

    return tool_names, tools


def parse_registry(registry_path: Path) -> dict[str, str]:
    """Parse registry.rs to get tool metadata (name -> category)."""
    content = registry_path.read_text()

    tools = {}

    # Find all m.insert("tool_name", Tool { ... category: Category::X, ... })
    pattern = r'm\.insert\("(\w+)", Tool \{[^}]+category: Category::(\w+),'
    matches = re.findall(pattern, content)

    for name, category in matches:
        tools[name] = category

    return tools


def map_swagger_to_mcp() -> dict[str, tuple[str, ...]]:
    """
    Manual mapping of swagger routes to MCP tool names.
    This is the source of truth for what should be implemented.
    Returns dict[swagger_path, tuple of (expected_tool_names)]
    """
    return {
        # Companies
        "/v1/companies/{cik}/calendar": ("get_company_calendar",),
        "/v1/companies/{id}/financials": ("get_company_financials",),
        "/v1/companies/{cik}/filings": ("get_company_filings",),

        # ETFs
        "/v1/etfs/{identifier}/holdings": ("get_etf_holdings",),

        # Forms 13F
        "/v1/forms/13f/{id}": ("get_form13f_submission",),
        "/v1/forms/13f": ("get_form13f_submissions",),

        # Forms 4
        "/v1/forms/4/{id}": ("get_form4_filing",),

        # Form ADV - Firms
        "/v1/forms/adv/firms": ("get_form_adv_firms",),
        "/v1/forms/adv/firms/{crd}": ("get_form_adv_firm",),
        "/v1/forms/adv/filings/{id}": ("get_form_adv_filing",),  # Individual filing detail

        # Form ADV - Firm Sub-resources (these map to existing registry tools)
        "/v1/forms/adv/firms/{crd}/filings": ("get_form_adv_firm_filings",),
        "/v1/forms/adv/firms/{crd}/addresses": ("get_form_adv_firm_addresses",),
        "/v1/forms/adv/firms/{crd}/notice_filings": ("get_form_adv_firm_notice_filings",),
        "/v1/forms/adv/firms/{crd}/direct_owners": ("get_form_adv_firm_direct_owners",),
        "/v1/forms/adv/firms/{crd}/indirect_owners": ("get_form_adv_firm_indirect_owners",),
        "/v1/forms/adv/firms/{crd}/ownership_chain": ("get_form_adv_firm_ownership_chain",),
        "/v1/forms/adv/firms/{crd}/private_funds": ("get_form_adv_firm_private_funds",),
        "/v1/forms/adv/firms/{crd}/related_persons": ("get_form_adv_firm_related_persons",),
        "/v1/forms/adv/firms/{crd}/other_names": ("get_form_adv_firm_other_names",),
        "/v1/forms/adv/firms/{crd}/sma_data": ("get_form_adv_firm_sma_data",),
        "/v1/forms/adv/firms/{crd}/disclosures": ("get_form_adv_firm_disclosures",),
        "/v1/forms/adv/firms/{crd}/brochures": ("get_form_adv_firm_brochures",),
        "/v1/forms/adv/firms/{crd}/aum_history": ("get_form_adv_firm_aum_history",),

        # Form ADV - Cross-entity searches
        "/v1/forms/adv/funds": ("get_form_adv_funds",),
        "/v1/forms/adv/owners": ("get_form_adv_owners",),

        # Lobbying
        "/v1/lobbying/client_performance": ("get_lobbying_client_performance",),
        "/v1/lobbying/clients/search": ("get_lobbying_clients_search",),
        "/v1/lobbying/clients/{id}": ("get_lobbying_client_detail",),

        # Lists (Watchlists) - GET and POST share the same path
        "/v1/lists": ("get_lists", "create_list"),
        "/v1/lists/{id_or_name}": ("get_list", "update_list", "delete_list"),

        # List Items
        "/v1/lists/{list_id}/items": ("add_list_item",),
        "/v1/lists/{list_id}/items/toggle": ("toggle_list_item",),
        "/v1/lists/{list_id}/items/{id}": ("update_list_item", "delete_list_item"),

        # Search
        "/v1/search": ("search",),

        # Documents (proxied through API)
        "/v1/documents/{accession_number}": ("get_sec_document",),
        "/v1/documents/{accession_number}/metadata": ("get_sec_document_metadata",),
    }


def main():
    root = Path(__file__).parent.parent
    swagger_path = root / "swagger.yaml"
    main_rs_path = root / "crates" / "mcp-server" / "src" / "main.rs"
    registry_path = root / "crates" / "core" / "src" / "tools" / "registry.rs"

    print("=" * 80)
    print("FilingExplorer API Coverage Report")
    print("=" * 80)
    print()

    # Parse swagger
    swagger_routes = parse_swagger(swagger_path)
    print(f"📄 Swagger Routes Found: {len(swagger_routes)}")

    # Parse MCP implementation
    implemented_tool_names, mcp_tools = parse_mcp_implementation(main_rs_path)
    print(f"🔧 MCP Tools Implemented (in execute_actual_tool): {len(implemented_tool_names)}")

    # Parse registry
    registry_tools = parse_registry(registry_path)
    print(f"📚 Registry Tools Defined: {len(registry_tools)}")
    print()

    # Get the mapping
    route_to_tools = map_swagger_to_mcp()

    # Categorize routes
    implemented_routes = []
    missing_routes = []
    unmapped_routes = []

    for route in swagger_routes:
        expected_tools = route_to_tools.get(route.path)

        if expected_tools is None:
            unmapped_routes.append(route)
        else:
            implemented = [t for t in expected_tools if t in implemented_tool_names]
            missing = [t for t in expected_tools if t not in implemented_tool_names]

            if missing:
                missing_routes.append((route, implemented, missing))
            else:
                implemented_routes.append((route, implemented))

    # Print implemented routes
    print("✅ IMPLEMENTED ROUTES")
    print("-" * 80)
    by_tag = {}
    for route, tools in implemented_routes:
        by_tag.setdefault(route.tag, []).append((route, tools))

    for tag in sorted(by_tag.keys()):
        print(f"\n  {tag}:")
        for route, tools in by_tag[tag]:
            tool_str = ", ".join(tools)
            print(f"    {route.method:6} {route.path}")
            print(f"           → {tool_str}")

    # Print missing routes
    if missing_routes:
        print("\n")
        print("❌ MISSING OR INCOMPLETE ROUTES")
        print("-" * 80)
        by_tag = {}
        for route, implemented, missing in missing_routes:
            by_tag.setdefault(route.tag, []).append((route, implemented, missing))

        for tag in sorted(by_tag.keys()):
            print(f"\n  {tag}:")
            for route, implemented, missing in by_tag[tag]:
                print(f"    {route.method:6} {route.path}")
                if implemented:
                    print(f"           ✓ Implemented: {', '.join(implemented)}")
                print(f"           ✗ Missing: {', '.join(missing)}")

    # Print unmapped routes (routes in swagger but not in our mapping)
    if unmapped_routes:
        print("\n")
        print("⚠️  UNMAPPED ROUTES (in swagger but no expected MCP tool defined)")
        print("-" * 80)
        by_tag = {}
        for route in unmapped_routes:
            by_tag.setdefault(route.tag, []).append(route)

        for tag in sorted(by_tag.keys()):
            print(f"\n  {tag}:")
            for route in by_tag[tag]:
                print(f"    {route.method:6} {route.path}")
                print(f"           {route.summary}")

    # Summary
    print("\n")
    print("=" * 80)
    print("SUMMARY")
    print("=" * 80)
    total_mapped = len(implemented_routes) + len(missing_routes)
    total_swagger = len(swagger_routes)
    implemented = len(implemented_routes)

    print(f"  Total Swagger Routes:     {total_swagger}")
    print(f"  Mapped to MCP Tools:      {total_mapped}")
    print(f"  Fully Implemented:        {implemented}")
    print(f"  Missing Implementation:   {len(missing_routes)}")
    print(f"  Unmapped (no tool):       {len(unmapped_routes)}")

    if total_mapped > 0:
        coverage = (implemented / total_mapped * 100)
        print(f"\n  Coverage (mapped only):   {coverage:.1f}%")

    print()

    # Check for tools in registry but not implemented
    registry_only = set(registry_tools.keys()) - implemented_tool_names
    if registry_only:
        print("⚠️  Tools in registry.rs but NOT in execute_actual_tool:")
        for tool in sorted(registry_only):
            cat = registry_tools.get(tool, "unknown")
            print(f"    - {tool} ({cat})")
        print()

    # Check for implemented tools not in registry
    impl_only = implemented_tool_names - set(registry_tools.keys())
    # Filter out meta-tools and search (which is a special tool)
    meta_tools = {"list_tool_categories", "search_tools", "execute_tool", "search"}
    impl_only = impl_only - meta_tools
    if impl_only:
        print("⚠️  Tools in execute_actual_tool but NOT in registry.rs:")
        for tool in sorted(impl_only):
            print(f"    - {tool}")
        print()

    # Print implemented tool list
    print("\n")
    print("=" * 80)
    print("IMPLEMENTED TOOLS IN execute_actual_tool")
    print("=" * 80)
    for tool in sorted(implemented_tool_names):
        in_registry = "✓" if tool in registry_tools else "✗"
        cat = registry_tools.get(tool, "n/a")
        print(f"  [{in_registry}] {tool} ({cat})")

    return len(missing_routes)


if __name__ == "__main__":
    missing = main()
    exit(0 if missing == 0 else 1)
