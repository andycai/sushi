# Graph Report - .  (2026-04-16)

## Corpus Check
- 129 files · ~91,176 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1444 nodes · 3100 edges · 86 communities detected
- Extraction: 93% EXTRACTED · 7% INFERRED · 0% AMBIGUOUS · INFERRED: 208 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Alpine.js Internal|Alpine.js Internal]]
- [[_COMMUNITY_KV Store Plugin UI|KV Store Plugin UI]]
- [[_COMMUNITY_TailwindCSS Internal|TailwindCSS Internal]]
- [[_COMMUNITY_HTMX Internal|HTMX Internal]]
- [[_COMMUNITY_Admin Web Tests|Admin Web Tests]]
- [[_COMMUNITY_Architecture and Core Concepts|Architecture and Core Concepts]]
- [[_COMMUNITY_Plugin Loader|Plugin Loader]]
- [[_COMMUNITY_Plugin Manager|Plugin Manager]]
- [[_COMMUNITY_Lua Bindings|Lua Bindings]]
- [[_COMMUNITY_Configuration Store|Configuration Store]]
- [[_COMMUNITY_Plugin Trait and Types|Plugin Trait and Types]]
- [[_COMMUNITY_Admin Menu Routes|Admin Menu Routes]]
- [[_COMMUNITY_Dashboard and Plugins UI|Dashboard and Plugins UI]]
- [[_COMMUNITY_Auth RBAC Core|Auth RBAC Core]]
- [[_COMMUNITY_Workspace Tabs UI|Workspace Tabs UI]]
- [[_COMMUNITY_Permissions CRUD UI|Permissions CRUD UI]]
- [[_COMMUNITY_Design Concepts and Specs|Design Concepts and Specs]]
- [[_COMMUNITY_Admin Registry|Admin Registry]]
- [[_COMMUNITY_Admin Roles Routes|Admin Roles Routes]]
- [[_COMMUNITY_Plans and Engineering Docs|Plans and Engineering Docs]]
- [[_COMMUNITY_Database Gateway|Database Gateway]]
- [[_COMMUNITY_Admin Menu JS|Admin Menu JS]]
- [[_COMMUNITY_Admin Permissions Routes|Admin Permissions Routes]]
- [[_COMMUNITY_Tracing Log Bridge|Tracing Log Bridge]]
- [[_COMMUNITY_API Router|API Router]]
- [[_COMMUNITY_Auth Models|Auth Models]]
- [[_COMMUNITY_Admin Router and Auth|Admin Router and Auth]]
- [[_COMMUNITY_Log Service|Log Service]]
- [[_COMMUNITY_JWT Service|JWT Service]]
- [[_COMMUNITY_SQLite Storage|SQLite Storage]]
- [[_COMMUNITY_Admin Users Routes|Admin Users Routes]]
- [[_COMMUNITY_User Repository|User Repository]]
- [[_COMMUNITY_Template Service|Template Service]]
- [[_COMMUNITY_Event Bus|Event Bus]]
- [[_COMMUNITY_API Users Routes|API Users Routes]]
- [[_COMMUNITY_Admin Plugins Routes|Admin Plugins Routes]]
- [[_COMMUNITY_Storage Abstraction|Storage Abstraction]]
- [[_COMMUNITY_Template Service Tests|Template Service Tests]]
- [[_COMMUNITY_Auth Middleware|Auth Middleware]]
- [[_COMMUNITY_Lua VM Sandbox|Lua VM Sandbox]]
- [[_COMMUNITY_Admin Workspace Routes|Admin Workspace Routes]]
- [[_COMMUNITY_API Auth Routes|API Auth Routes]]
- [[_COMMUNITY_Admin Login Routes|Admin Login Routes]]
- [[_COMMUNITY_Lua Module Loader|Lua Module Loader]]
- [[_COMMUNITY_CLI App Bootstrap|CLI App Bootstrap]]
- [[_COMMUNITY_Template Rendering|Template Rendering]]
- [[_COMMUNITY_DB Gateway Tests|DB Gateway Tests]]
- [[_COMMUNITY_CLI Config Command|CLI Config Command]]
- [[_COMMUNITY_CLI Plugin Command|CLI Plugin Command]]
- [[_COMMUNITY_Sushi Context|Sushi Context]]
- [[_COMMUNITY_CLI Main Entry|CLI Main Entry]]
- [[_COMMUNITY_Frontend Frameworks|Frontend Frameworks]]
- [[_COMMUNITY_CLI Serve Command|CLI Serve Command]]
- [[_COMMUNITY_CLI Run Command|CLI Run Command]]
- [[_COMMUNITY_CLI Seed Command|CLI Seed Command]]
- [[_COMMUNITY_Admin Config Routes|Admin Config Routes]]
- [[_COMMUNITY_Admin Logs Routes|Admin Logs Routes]]
- [[_COMMUNITY_Plugin Init Scripts|Plugin Init Scripts]]
- [[_COMMUNITY_KV Store DB Module|KV Store DB Module]]
- [[_COMMUNITY_KV Store JSON Module|KV Store JSON Module]]
- [[_COMMUNITY_KV Store Form Module|KV Store Form Module]]
- [[_COMMUNITY_KV Store Domain Layer|KV Store Domain Layer]]
- [[_COMMUNITY_KV Store API Interface|KV Store API Interface]]
- [[_COMMUNITY_RBAC Navigation Model|RBAC Navigation Model]]
- [[_COMMUNITY_Admin Dashboard Page|Admin Dashboard Page]]
- [[_COMMUNITY_Template Error Type|Template Error Type]]
- [[_COMMUNITY_KV Store Bootstrap|KV Store Bootstrap]]
- [[_COMMUNITY_KV Store HTML Utils|KV Store HTML Utils]]
- [[_COMMUNITY_KV Store CLI Interface|KV Store CLI Interface]]
- [[_COMMUNITY_KV Store Admin Interface|KV Store Admin Interface]]
- [[_COMMUNITY_Platform and Namesake|Platform and Namesake]]
- [[_COMMUNITY_DB Migration and RBAC|DB Migration and RBAC]]
- [[_COMMUNITY_Favicon Design|Favicon Design]]
- [[_COMMUNITY_JWT and RBAC Concepts|JWT and RBAC Concepts]]
- [[_COMMUNITY_Su Shi Favicon|Su Shi Favicon]]
- [[_COMMUNITY_CLI Library|CLI Library]]
- [[_COMMUNITY_CLI Commands Module|CLI Commands Module]]
- [[_COMMUNITY_Admin Library|Admin Library]]
- [[_COMMUNITY_Admin Routes Module|Admin Routes Module]]
- [[_COMMUNITY_Core Library|Core Library]]
- [[_COMMUNITY_Auth Module|Auth Module]]
- [[_COMMUNITY_Web Module|Web Module]]
- [[_COMMUNITY_Lua Module|Lua Module]]
- [[_COMMUNITY_DB Module|DB Module]]
- [[_COMMUNITY_API Library|API Library]]
- [[_COMMUNITY_API Routes Module|API Routes Module]]

## God Nodes (most connected - your core abstractions)
1. `build_app()` - 38 edges
2. `He()` - 37 edges
3. `te()` - 31 edges
4. `get()` - 30 edges
5. `inject_sushi_api()` - 29 edges
6. `gt()` - 29 edges
7. `ie()` - 29 edges
8. `Ae()` - 29 edges
9. `add()` - 27 edges
10. `ce()` - 25 edges

## Surprising Connections (you probably didn't know these)
- `sushi-core Crate` --implements--> `Authentication & RBAC`  [INFERRED]
  AGENTS.md → docs/wiki/architecture/auth-rbac.md
- `sushi-core Crate` --implements--> `EventBus Architecture`  [INFERRED]
  AGENTS.md → docs/wiki/architecture/README.md
- `sushi Context Object` --references--> `sushi.config Lua Namespace`  [INFERRED]
  AGENTS.md → docs/wiki/lua-api/sushi.config.md
- `notifyFeedback()` --calls--> `consumeFeedback()`  [INFERRED]
  plugins/official/kv-store/web/static/kv.js → web/static/admin/js/ui-kit.js
- `notifyFeedback()` --calls--> `notify()`  [INFERRED]
  plugins/official/kv-store/web/static/kv.js → web/static/admin/js/ui-kit.js

## Hyperedges (group relationships)
- **All sushi.* Lua API Namespaces** — sushi_api_namespace, sushi_admin_namespace, sushi_cli_namespace, sushi_config_namespace, sushi_log_namespace, sushi_db_namespace, sushi_web_namespace, sushi_event_namespace, sushi_json_namespace, sushi_auth_namespace [EXTRACTED 1.00]
- **Permission-Gated Lua APIs** — sushi_api_namespace, sushi_admin_namespace, sushi_cli_namespace, sushi_db_namespace, sushi_web_namespace [EXTRACTED 1.00]
- **Plugin Registration Flow (init.lua to internal tables)** — sushi_context_object, internal_pending_routes, internal_pending_commands, internal_pending_pages, internal_handlers, internal_event_handlers [INFERRED 0.85]
- **Sushi Plugin Ecosystem Components** — concept_plugin_trait, concept_plugin_manifest, concept_plugin_tiering, concept_plugin_path_id, concept_effective_permissions, concept_secure_lua_module_loader, concept_plugin_asset_isolation, concept_admin_asset_bundles [INFERRED 0.80]
- **Admin UI Architecture Progression** — concept_admin_menu_system, concept_htmx_partial_loading, concept_admin_workspace_tabs, concept_workspace_assets_api, concept_no_cdn_policy [INFERRED 0.75]
- **KV Store Plugin Modernization Layers** — concept_kv_layered_arch, concept_kv_error_taxonomy, concept_secure_lua_module_loader, concept_plugin_tiering, concept_plugin_asset_isolation [INFERRED 0.80]

## Communities

### Community 0 - "Alpine.js Internal"
Cohesion: 0.03
Nodes (154): _(), A(), ae(), ai(), an(), ao(), ar(), At() (+146 more)

### Community 1 - "KV Store Plugin UI"
Cohesion: 0.02
Nodes (63): init(), closeDeleteConfirm(), closeModal(), isErrorFeedback(), isSuccessfulKvRequest(), notifyFeedback(), onDeleteAfterRequest(), onUpsertAfterRequest() (+55 more)

### Community 2 - "TailwindCSS Internal"
Cohesion: 0.05
Nodes (112): ownKeys(), raw(), r(), $(), a(), add(), addKeyframes(), ae() (+104 more)

### Community 3 - "HTMX Internal"
Cohesion: 0.1
Nodes (103): $(), a(), Ae(), an(), at(), B(), be(), bn() (+95 more)

### Community 4 - "Admin Web Tests"
Cohesion: 0.08
Nodes (63): admin_bearer_token(), admin_can_crud_permissions_via_partials(), admin_can_crud_roles_and_assign_permissions(), admin_prefix_is_rejected_for_static(), admin_requires_auth_without_token(), all_admin_templates_exclude_external_cdn_links(), assert_no_external_assets_in_html(), base_template_has_no_plugin_specific_module_mappings() (+55 more)

### Community 5 - "Architecture and Core Concepts"
Cohesion: 0.06
Nodes (50): Architecture Overview, Argon2 Password Hashing, Authentication & RBAC, Axum Web Framework, Built-in Events, Configuration Guide, Database Layer, DbGateway (+42 more)

### Community 6 - "Plugin Loader"
Cohesion: 0.09
Nodes (21): create_plugin_dir(), LuaPlugin, normalize_static_url_prefix(), page_assets_fail_when_file_missing(), page_assets_resolve_bundle_then_page_assets(), parse_optional_string_array(), parse_page_assets_entry(), push_resolved_assets() (+13 more)

### Community 7 - "Plugin Manager"
Cohesion: 0.1
Nodes (13): admin_page_assets_are_stored_and_returned(), AdminHandlerBinding, db_permission_name(), list_admin_pages_for_plugin_returns_titles(), list_plugin_static_roots_returns_sorted_entries(), PageResolvedAssets, PluginAdminPageInfo, PluginInfo (+5 more)

### Community 8 - "Lua Bindings"
Cohesion: 0.14
Nodes (33): build_web_context(), inject_sushi_api(), lua_params(), map_db_permission(), next_handler_key(), parse_asset_string_array(), parse_page_assets(), test_api_route_registration() (+25 more)

### Community 9 - "Configuration Store"
Cohesion: 0.1
Nodes (22): ConfigStore, DatabaseConfig, default_access_ttl(), default_body_size_limit(), default_db_path(), default_host(), default_jwt_secret(), default_plugins_dir() (+14 more)

### Community 10 - "Plugin Trait and Types"
Cohesion: 0.07
Nodes (17): DatabasePermission, FnPlugin, Permissions, Plugin, PluginAdminAssetsConfig, PluginAdminConfig, PluginAssetBundle, PluginError (+9 more)

### Community 11 - "Admin Menu Routes"
Cohesion: 0.13
Nodes (26): create_menu_item(), CreateMenuItem, delete_menu_item(), ensure_menu_schema(), flash_response(), flash_response_with_trigger(), is_system_route(), list_menu_items() (+18 more)

### Community 12 - "Dashboard and Plugins UI"
Cohesion: 0.11
Nodes (20): init(), refreshPulse(), addRecent(), canUseStorage(), collectPages(), init(), isPinned(), markLoaded() (+12 more)

### Community 13 - "Auth RBAC Core"
Cohesion: 0.1
Nodes (8): Permission, PermissionSummary, RbacRepository, Role, RolePermissionAssignment, RoleSummary, row_to_permission(), row_to_role()

### Community 14 - "Workspace Tabs UI"
Cohesion: 0.18
Nodes (26): activateTab(), bootstrapInitialPane(), canUseStorage(), closeTab(), createPane(), emitChange(), ensureDashboardTab(), ensurePane() (+18 more)

### Community 15 - "Permissions CRUD UI"
Cohesion: 0.12
Nodes (11): closeDelete(), closeEditor(), isErrorFeedback(), isSuccessfulRequest(), onDeleteAfterRequest(), onEditorAfterRequest(), openDelete(), openEdit() (+3 more)

### Community 16 - "Design Concepts and Specs"
Cohesion: 0.09
Nodes (24): Admin Asset Bundles Declaration, Admin Dynamic Menu System, Admin Workspace Tabbed Navigation, DbGateway (Permissioned SQL), Database Permission Tiers (read/write/admin), Effective Permissions (Official Override), EventBus (Plugin Inter-Communication), HTMX Partial Content Loading (+16 more)

### Community 17 - "Admin Registry"
Cohesion: 0.14
Nodes (10): AdminPageEntry, AdminRegistry, AdminWidgetEntry, ApiRegistry, CliCommandEntry, CliRegistry, RouteEntry, test_admin_registry() (+2 more)

### Community 18 - "Admin Roles Routes"
Cohesion: 0.2
Nodes (15): CreateRoleForm, flash_response(), flash_response_with_trigger(), render_roles_rows(), role_permissions_form_partial(), role_permissions_update_partial(), roles_create_partial(), roles_delete_partial() (+7 more)

### Community 19 - "Plans and Engineering Docs"
Cohesion: 0.15
Nodes (19): Admin Menu Design, Admin Menu Refactor Implementation Plan, Admin UI Redesign Design, Admin UI Redesign Implementation Plan, Admin Workspace Tabs + HTMX Partial Navigation Design, Admin Workspace Tabs Implementation Plan, Sushi Coding Standards, Engineering Standards Index (+11 more)

### Community 20 - "Database Gateway"
Cohesion: 0.18
Nodes (10): classify_keyword(), classify_sql(), DbGateway, DbGatewayError, DbPermission, first_statement_keyword(), has_multiple_statements(), is_token_char() (+2 more)

### Community 21 - "Admin Menu JS"
Cohesion: 0.21
Nodes (14): expandForRoute(), getChildren(), handleMenuClick(), hasActiveDescendant(), hasChildren(), init(), isActive(), loadMenu() (+6 more)

### Community 22 - "Admin Permissions Routes"
Cohesion: 0.25
Nodes (15): CreatePermissionForm, flash_response(), flash_response_with_trigger(), permissions_create_partial(), permissions_delete_partial(), permissions_table_partial(), permissions_update_partial(), render_permissions_rows() (+7 more)

### Community 23 - "Tracing Log Bridge"
Cohesion: 0.21
Nodes (7): active_log_service_cell(), bridge_layer_collects_warn_and_error_only(), current_log_service(), EventVisitor, layer(), LogServiceBridgeLayer, register_log_service()

### Community 24 - "API Router"
Cohesion: 0.24
Nodes (13): build_app(), parse_status_envelope(), plugin_api_dispatch(), PluginApiState, test_build_app_accepts_cookie_token_for_users_route(), test_build_app_allows_login_without_token(), test_build_app_requires_auth_for_users_route(), test_context() (+5 more)

### Community 25 - "Auth Models"
Cohesion: 0.19
Nodes (5): custom_role_round_trip_uses_slug(), LoginRequest, TokenResponse, User, UserRole

### Community 26 - "Admin Router and Auth"
Cohesion: 0.26
Nodes (9): admin_auth_middleware(), admin_path_matches(), AdminAuthState, append_assets_to_html_response(), build_admin_router(), is_plugin_workspace_root_path(), is_valid_plugin_mount_id(), matches_static_prefix() (+1 more)

### Community 27 - "Log Service"
Cohesion: 0.24
Nodes (3): Bn(), LogEntry, LogService

### Community 28 - "JWT Service"
Cohesion: 0.35
Nodes (5): Claims, JwtService, test_create_and_verify_access_token(), test_invalid_token(), test_refresh_token_type()

### Community 29 - "SQLite Storage"
Cohesion: 0.44
Nodes (5): SqliteStorage, test_run_migrations(), test_sqlite_execute_and_query(), test_sqlite_multiple_rows(), test_sqlite_null_handling()

### Community 30 - "Admin Users Routes"
Cohesion: 0.38
Nodes (8): CreateUserForm, flash_response(), flash_response_with_trigger(), render_users_rows(), users_create_partial(), users_delete_partial(), users_table_partial(), validate_create_user_form()

### Community 31 - "User Repository"
Cohesion: 0.27
Nodes (2): row_to_user(), UserRepository

### Community 32 - "Template Service"
Cohesion: 0.4
Nodes (6): load_template(), safe_join(), split_plugin_template_name(), TemplateService, validate_root(), validate_root_optional()

### Community 33 - "Event Bus"
Cohesion: 0.47
Nodes (4): EventBus, test_emit_no_subscribers(), test_multiple_subscribers(), test_subscribe_and_emit()

### Community 34 - "API Users Routes"
Cohesion: 0.22
Nodes (4): create_user(), CreateUserRequest, PaginationParams, UsersRouteState

### Community 35 - "Admin Plugins Routes"
Cohesion: 0.33
Nodes (6): find_plugin(), plugin_pages_api(), plugin_workspace_context(), plugin_workspace_page(), PluginWorkspaceResponse, render_plugin_workspace_partial()

### Community 36 - "Storage Abstraction"
Cohesion: 0.22
Nodes (5): sqlite_to_json(), Storage, StorageConn, StorageConn<'a>, StorageError

### Community 37 - "Template Service Tests"
Cohesion: 0.25
Nodes (0): 

### Community 38 - "Auth Middleware"
Cohesion: 0.39
Nodes (7): admin_can_access_admin_partials(), auth_state(), AuthState, AuthUser, extract_token_from_cookie(), non_admin_cannot_access_admin_partials(), require_auth()

### Community 39 - "Lua VM Sandbox"
Cohesion: 0.46
Nodes (7): create_sandboxed_vm(), test_sandbox_allows_basic_lua(), test_sandbox_allows_string_ops(), test_sandbox_allows_tables(), test_sandbox_blocks_io(), test_sandbox_blocks_os_execute(), test_sandbox_blocks_require()

### Community 40 - "Admin Workspace Routes"
Cohesion: 0.38
Nodes (4): module_template(), module_to_admin_path(), workspace_partial(), WorkspaceAssetsResponse

### Community 41 - "API Auth Routes"
Cohesion: 0.29
Nodes (2): AuthRouteState, RefreshRequest

### Community 42 - "Admin Login Routes"
Cohesion: 0.47
Nodes (4): login_error_response(), login_submit(), LoginForm, render_login_flash_html()

### Community 43 - "Lua Module Loader"
Cohesion: 0.6
Nodes (5): install_plugin_require(), require_loads_plugin_local_module(), require_rejects_parent_traversal(), safe_module_join(), validate_module_name()

### Community 44 - "CLI App Bootstrap"
Cohesion: 0.8
Nodes (4): bootstrap(), resolve_dir(), resolve_static_dir(), resolve_templates_dir()

### Community 45 - "Template Rendering"
Cohesion: 0.7
Nodes (4): merge_static_prefix(), normalize_static_url_prefix(), render_template(), render_template_with_context()

### Community 46 - "DB Gateway Tests"
Cohesion: 0.4
Nodes (0): 

### Community 47 - "CLI Config Command"
Cohesion: 0.5
Nodes (2): ConfigArgs, ConfigCommand

### Community 48 - "CLI Plugin Command"
Cohesion: 0.5
Nodes (2): PluginArgs, PluginCommand

### Community 49 - "Sushi Context"
Cohesion: 0.5
Nodes (1): SushiContext

### Community 50 - "CLI Main Entry"
Cohesion: 0.5
Nodes (2): Cli, Commands

### Community 51 - "Frontend Frameworks"
Cohesion: 0.5
Nodes (4): Admin Panel, Alpine.js Framework, HTMX Framework, TailwindCSS Framework

### Community 52 - "CLI Serve Command"
Cohesion: 0.67
Nodes (1): ServeArgs

### Community 53 - "CLI Run Command"
Cohesion: 0.67
Nodes (1): RunArgs

### Community 54 - "CLI Seed Command"
Cohesion: 0.67
Nodes (1): SeedArgs

### Community 55 - "Admin Config Routes"
Cohesion: 0.67
Nodes (0): 

### Community 56 - "Admin Logs Routes"
Cohesion: 0.67
Nodes (0): 

### Community 57 - "Plugin Init Scripts"
Cohesion: 0.67
Nodes (1): sushi.init()

### Community 58 - "KV Store DB Module"
Cohesion: 0.67
Nodes (0): 

### Community 59 - "KV Store JSON Module"
Cohesion: 0.67
Nodes (0): 

### Community 60 - "KV Store Form Module"
Cohesion: 1.0
Nodes (2): M.parse_urlencoded(), url_decode()

### Community 61 - "KV Store Domain Layer"
Cohesion: 0.67
Nodes (0): 

### Community 62 - "KV Store API Interface"
Cohesion: 0.67
Nodes (0): 

### Community 63 - "RBAC Navigation Model"
Cohesion: 0.67
Nodes (3): Admin Workspace RBAC Mapping, RBAC Permission Model, Workspace Navigation (HTMX + Tabs)

### Community 64 - "Admin Dashboard Page"
Cohesion: 1.0
Nodes (0): 

### Community 65 - "Template Error Type"
Cohesion: 1.0
Nodes (1): TemplateError

### Community 66 - "KV Store Bootstrap"
Cohesion: 1.0
Nodes (0): 

### Community 67 - "KV Store HTML Utils"
Cohesion: 1.0
Nodes (0): 

### Community 68 - "KV Store CLI Interface"
Cohesion: 1.0
Nodes (0): 

### Community 69 - "KV Store Admin Interface"
Cohesion: 1.0
Nodes (0): 

### Community 70 - "Platform and Namesake"
Cohesion: 1.0
Nodes (2): Su Shi (苏轼) - Project Namesake, Sushi Platform

### Community 71 - "DB Migration and RBAC"
Cohesion: 1.0
Nodes (2): Database Migration Tables, RBAC Data Model (SQL)

### Community 72 - "Favicon Design"
Cohesion: 1.0
Nodes (2): Sushi Favicon Su Shi Minimal Ink Design, Su Shi Minimal Ink Favicon Implementation Plan

### Community 73 - "JWT and RBAC Concepts"
Cohesion: 1.0
Nodes (2): JWT Authentication (access + refresh tokens), RBAC (Role-Based Access Control)

### Community 74 - "Su Shi Favicon"
Cohesion: 1.0
Nodes (2): Su Shi Minimal Ink Mark (Poetry Abstract), Favicon SVG (Su Shi Minimal Ink Mark)

### Community 75 - "CLI Library"
Cohesion: 1.0
Nodes (0): 

### Community 76 - "CLI Commands Module"
Cohesion: 1.0
Nodes (0): 

### Community 77 - "Admin Library"
Cohesion: 1.0
Nodes (0): 

### Community 78 - "Admin Routes Module"
Cohesion: 1.0
Nodes (0): 

### Community 79 - "Core Library"
Cohesion: 1.0
Nodes (0): 

### Community 80 - "Auth Module"
Cohesion: 1.0
Nodes (0): 

### Community 81 - "Web Module"
Cohesion: 1.0
Nodes (0): 

### Community 82 - "Lua Module"
Cohesion: 1.0
Nodes (0): 

### Community 83 - "DB Module"
Cohesion: 1.0
Nodes (0): 

### Community 84 - "API Library"
Cohesion: 1.0
Nodes (0): 

### Community 85 - "API Routes Module"
Cohesion: 1.0
Nodes (0): 

## Knowledge Gaps
- **105 isolated node(s):** `ServeArgs`, `ConfigArgs`, `ConfigCommand`, `RunArgs`, `PluginArgs` (+100 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **Thin community `Admin Dashboard Page`** (2 nodes): `dashboard.rs`, `dashboard_page()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Template Error Type`** (2 nodes): `template_error.rs`, `TemplateError`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `KV Store Bootstrap`** (2 nodes): `register.lua`, `M.register()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `KV Store HTML Utils`** (2 nodes): `M.escape()`, `html.lua`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `KV Store CLI Interface`** (2 nodes): `M.new()`, `cli.lua`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `KV Store Admin Interface`** (2 nodes): `M.new()`, `admin.lua`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Platform and Namesake`** (2 nodes): `Su Shi (苏轼) - Project Namesake`, `Sushi Platform`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `DB Migration and RBAC`** (2 nodes): `Database Migration Tables`, `RBAC Data Model (SQL)`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Favicon Design`** (2 nodes): `Sushi Favicon Su Shi Minimal Ink Design`, `Su Shi Minimal Ink Favicon Implementation Plan`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `JWT and RBAC Concepts`** (2 nodes): `JWT Authentication (access + refresh tokens)`, `RBAC (Role-Based Access Control)`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Su Shi Favicon`** (2 nodes): `Su Shi Minimal Ink Mark (Poetry Abstract)`, `Favicon SVG (Su Shi Minimal Ink Mark)`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `CLI Library`** (1 nodes): `lib.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `CLI Commands Module`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Admin Library`** (1 nodes): `lib.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Admin Routes Module`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Core Library`** (1 nodes): `lib.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Auth Module`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Web Module`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Lua Module`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `DB Module`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `API Library`** (1 nodes): `lib.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `API Routes Module`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `add()` connect `TailwindCSS Internal` to `Alpine.js Internal`, `KV Store Plugin UI`, `HTMX Internal`, `Workspace Tabs UI`, `Admin Menu JS`?**
  _High betweenness centrality (0.096) - this node is a cross-community bridge._
- **Why does `dismissToast()` connect `KV Store Plugin UI` to `TailwindCSS Internal`?**
  _High betweenness centrality (0.067) - this node is a cross-community bridge._
- **Why does `init()` connect `Alpine.js Internal` to `HTMX Internal`, `Permissions CRUD UI`?**
  _High betweenness centrality (0.023) - this node is a cross-community bridge._
- **Are the 2 inferred relationships involving `He()` (e.g. with `set()` and `keys()`) actually correct?**
  _`He()` has 2 INFERRED edges - model-reasoned connections that need verification._
- **What connects `ServeArgs`, `ConfigArgs`, `ConfigCommand` to the rest of the system?**
  _105 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Alpine.js Internal` be split into smaller, more focused modules?**
  _Cohesion score 0.03 - nodes in this community are weakly interconnected._
- **Should `KV Store Plugin UI` be split into smaller, more focused modules?**
  _Cohesion score 0.02 - nodes in this community are weakly interconnected._