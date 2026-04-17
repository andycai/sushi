# Graph Report - .  (2026-04-17)

## Corpus Check
- 104 files · ~114,205 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1593 nodes · 3378 edges · 95 communities detected
- Extraction: 93% EXTRACTED · 7% INFERRED · 0% AMBIGUOUS · INFERRED: 220 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Community 0|Community 0]]
- [[_COMMUNITY_Community 1|Community 1]]
- [[_COMMUNITY_Community 2|Community 2]]
- [[_COMMUNITY_Community 3|Community 3]]
- [[_COMMUNITY_Community 4|Community 4]]
- [[_COMMUNITY_Community 5|Community 5]]
- [[_COMMUNITY_Community 6|Community 6]]
- [[_COMMUNITY_Community 7|Community 7]]
- [[_COMMUNITY_Community 8|Community 8]]
- [[_COMMUNITY_Community 9|Community 9]]
- [[_COMMUNITY_Community 10|Community 10]]
- [[_COMMUNITY_Community 11|Community 11]]
- [[_COMMUNITY_Community 12|Community 12]]
- [[_COMMUNITY_Community 13|Community 13]]
- [[_COMMUNITY_Community 14|Community 14]]
- [[_COMMUNITY_Community 15|Community 15]]
- [[_COMMUNITY_Community 16|Community 16]]
- [[_COMMUNITY_Community 17|Community 17]]
- [[_COMMUNITY_Community 18|Community 18]]
- [[_COMMUNITY_Community 19|Community 19]]
- [[_COMMUNITY_Community 20|Community 20]]
- [[_COMMUNITY_Community 21|Community 21]]
- [[_COMMUNITY_Community 22|Community 22]]
- [[_COMMUNITY_Community 23|Community 23]]
- [[_COMMUNITY_Community 24|Community 24]]
- [[_COMMUNITY_Community 25|Community 25]]
- [[_COMMUNITY_Community 26|Community 26]]
- [[_COMMUNITY_Community 27|Community 27]]
- [[_COMMUNITY_Community 28|Community 28]]
- [[_COMMUNITY_Community 29|Community 29]]
- [[_COMMUNITY_Community 30|Community 30]]
- [[_COMMUNITY_Community 31|Community 31]]
- [[_COMMUNITY_Community 32|Community 32]]
- [[_COMMUNITY_Community 33|Community 33]]
- [[_COMMUNITY_Community 34|Community 34]]
- [[_COMMUNITY_Community 35|Community 35]]
- [[_COMMUNITY_Community 36|Community 36]]
- [[_COMMUNITY_Community 37|Community 37]]
- [[_COMMUNITY_Community 38|Community 38]]
- [[_COMMUNITY_Community 39|Community 39]]
- [[_COMMUNITY_Community 40|Community 40]]
- [[_COMMUNITY_Community 41|Community 41]]
- [[_COMMUNITY_Community 42|Community 42]]
- [[_COMMUNITY_Community 43|Community 43]]
- [[_COMMUNITY_Community 44|Community 44]]
- [[_COMMUNITY_Community 45|Community 45]]
- [[_COMMUNITY_Community 46|Community 46]]
- [[_COMMUNITY_Community 47|Community 47]]
- [[_COMMUNITY_Community 48|Community 48]]
- [[_COMMUNITY_Community 49|Community 49]]
- [[_COMMUNITY_Community 50|Community 50]]
- [[_COMMUNITY_Community 51|Community 51]]
- [[_COMMUNITY_Community 52|Community 52]]
- [[_COMMUNITY_Community 53|Community 53]]
- [[_COMMUNITY_Community 54|Community 54]]
- [[_COMMUNITY_Community 55|Community 55]]
- [[_COMMUNITY_Community 56|Community 56]]
- [[_COMMUNITY_Community 57|Community 57]]
- [[_COMMUNITY_Community 58|Community 58]]
- [[_COMMUNITY_Community 59|Community 59]]
- [[_COMMUNITY_Community 60|Community 60]]
- [[_COMMUNITY_Community 61|Community 61]]
- [[_COMMUNITY_Community 62|Community 62]]
- [[_COMMUNITY_Community 63|Community 63]]
- [[_COMMUNITY_Community 64|Community 64]]
- [[_COMMUNITY_Community 65|Community 65]]
- [[_COMMUNITY_Community 66|Community 66]]
- [[_COMMUNITY_Community 67|Community 67]]
- [[_COMMUNITY_Community 68|Community 68]]
- [[_COMMUNITY_Community 69|Community 69]]
- [[_COMMUNITY_Community 70|Community 70]]
- [[_COMMUNITY_Community 71|Community 71]]
- [[_COMMUNITY_Community 72|Community 72]]
- [[_COMMUNITY_Community 73|Community 73]]
- [[_COMMUNITY_Community 74|Community 74]]
- [[_COMMUNITY_Community 75|Community 75]]
- [[_COMMUNITY_Community 76|Community 76]]
- [[_COMMUNITY_Community 77|Community 77]]
- [[_COMMUNITY_Community 78|Community 78]]
- [[_COMMUNITY_Community 79|Community 79]]
- [[_COMMUNITY_Community 80|Community 80]]
- [[_COMMUNITY_Community 81|Community 81]]
- [[_COMMUNITY_Community 82|Community 82]]
- [[_COMMUNITY_Community 83|Community 83]]
- [[_COMMUNITY_Community 84|Community 84]]
- [[_COMMUNITY_Community 85|Community 85]]
- [[_COMMUNITY_Community 86|Community 86]]
- [[_COMMUNITY_Community 87|Community 87]]
- [[_COMMUNITY_Community 88|Community 88]]
- [[_COMMUNITY_Community 89|Community 89]]
- [[_COMMUNITY_Community 90|Community 90]]
- [[_COMMUNITY_Community 91|Community 91]]
- [[_COMMUNITY_Community 92|Community 92]]
- [[_COMMUNITY_Community 93|Community 93]]
- [[_COMMUNITY_Community 94|Community 94]]

## God Nodes (most connected - your core abstractions)
1. `He()` - 37 edges
2. `build_app()` - 35 edges
3. `PluginManager` - 32 edges
4. `inject_sushi_api()` - 31 edges
5. `te()` - 31 edges
6. `get()` - 30 edges
7. `gt()` - 29 edges
8. `ie()` - 29 edges
9. `Ae()` - 29 edges
10. `add()` - 27 edges

## Surprising Connections (you probably didn't know these)
- `refreshPages()` --calls--> `refreshPartial()`  [INFERRED]
  plugins/official/cms/web/static/cms.js → web/static/admin/js/ui-kit.js
- `sushi-core Crate` --implements--> `Authentication & RBAC`  [INFERRED]
  AGENTS.md → docs/wiki/architecture/auth-rbac.md
- `sushi-core Crate` --implements--> `EventBus Architecture`  [INFERRED]
  AGENTS.md → docs/wiki/architecture/README.md
- `sushi.config Lua Namespace` --references--> `sushi Context Object`  [INFERRED]
  docs/wiki/lua-api/sushi.config.md → AGENTS.md
- `notifyFeedback()` --calls--> `consumeFeedback()`  [INFERRED]
  plugins/official/kv-store/web/static/kv.js → web/static/admin/js/ui-kit.js

## Hyperedges (group relationships)
- **All sushi.* Lua API Namespaces** — sushi_api_namespace, sushi_admin_namespace, sushi_cli_namespace, sushi_config_namespace, sushi_log_namespace, sushi_db_namespace, sushi_web_namespace, sushi_event_namespace, sushi_json_namespace, sushi_auth_namespace [EXTRACTED 1.00]
- **Permission-Gated Lua APIs** — sushi_api_namespace, sushi_admin_namespace, sushi_cli_namespace, sushi_db_namespace, sushi_web_namespace [EXTRACTED 1.00]
- **Plugin Registration Flow (init.lua to internal tables)** — sushi_context_object, internal_pending_routes, internal_pending_commands, internal_pending_pages, internal_handlers, internal_event_handlers [INFERRED 0.85]
- **Sushi Plugin Ecosystem Components** — concept_plugin_trait, concept_plugin_manifest, concept_plugin_tiering, concept_plugin_path_id, concept_effective_permissions, concept_secure_lua_module_loader, concept_plugin_asset_isolation, concept_admin_asset_bundles [INFERRED 0.80]
- **Admin UI Architecture Progression** — concept_admin_menu_system, concept_htmx_partial_loading, concept_admin_workspace_tabs, concept_workspace_assets_api, concept_no_cdn_policy [INFERRED 0.75]
- **KV Store Plugin Modernization Layers** — concept_kv_layered_arch, concept_kv_error_taxonomy, concept_secure_lua_module_loader, concept_plugin_tiering, concept_plugin_asset_isolation [INFERRED 0.80]

## Communities

### Community 0 - "Community 0"
Cohesion: 0.03
Nodes (157): _(), A(), ae(), ai(), an(), ao(), ar(), At() (+149 more)

### Community 1 - "Community 1"
Cohesion: 0.02
Nodes (81): refreshPages(), init(), closeDeleteConfirm(), closeModal(), isErrorFeedback(), isSuccessfulKvRequest(), notifyFeedback(), onDeleteAfterRequest() (+73 more)

### Community 2 - "Community 2"
Cohesion: 0.05
Nodes (110): ownKeys(), raw(), r(), $(), a(), add(), addKeyframes(), ae() (+102 more)

### Community 3 - "Community 3"
Cohesion: 0.1
Nodes (103): $(), a(), Ae(), an(), at(), B(), be(), bn() (+95 more)

### Community 4 - "Community 4"
Cohesion: 0.07
Nodes (70): admin_bearer_token(), admin_can_crud_permissions_via_partials(), admin_can_crud_roles_and_assign_permissions(), admin_cms_category_delete_returns_flash_on_conflict(), admin_cms_workspace_page_renders(), admin_http_bindings(), admin_prefix_is_rejected_for_static(), admin_requires_auth_without_token() (+62 more)

### Community 5 - "Community 5"
Cohesion: 0.08
Nodes (19): admin_page_assets_are_stored_and_returned(), AdminHandlerBinding, api_route_policy_matches_wildcard_for_concrete_path(), ApiHandlerBinding, call_api_handler_matches_wildcards(), CliHandlerBinding, db_permission_name(), list_admin_pages_for_plugin_returns_titles() (+11 more)

### Community 6 - "Community 6"
Cohesion: 0.06
Nodes (50): Architecture Overview, Argon2 Password Hashing, Authentication & RBAC, Axum Web Framework, Built-in Events, Configuration Guide, Database Layer, DbGateway (+42 more)

### Community 7 - "Community 7"
Cohesion: 0.08
Nodes (27): create_plugin_dir(), LuaPlugin, normalize_static_url_prefix(), page_assets_fail_when_file_missing(), page_assets_resolve_bundle_then_page_assets(), parse_entry_policy(), parse_optional_string_array(), parse_page_assets_entry() (+19 more)

### Community 8 - "Community 8"
Cohesion: 0.13
Nodes (35): build_web_context(), inject_sushi_api(), lua_params(), map_db_permission(), next_handler_key(), parse_asset_string_array(), parse_optional_policy(), parse_page_assets() (+27 more)

### Community 9 - "Community 9"
Cohesion: 0.07
Nodes (18): DatabasePermission, FnPlugin, Permissions, Plugin, PluginAdminAssetsConfig, PluginAdminConfig, PluginAssetBundle, PluginError (+10 more)

### Community 10 - "Community 10"
Cohesion: 0.1
Nodes (22): ConfigStore, DatabaseConfig, default_access_ttl(), default_body_size_limit(), default_db_path(), default_host(), default_jwt_secret(), default_plugins_dir() (+14 more)

### Community 11 - "Community 11"
Cohesion: 0.1
Nodes (10): Permission, PermissionSummary, RbacRepository, replace_role_permissions_syncs_role_policy_keys(), repo_with_schema(), Role, RolePermissionAssignment, RoleSummary (+2 more)

### Community 12 - "Community 12"
Cohesion: 0.13
Nodes (26): create_menu_item(), CreateMenuItem, delete_menu_item(), ensure_menu_schema(), flash_response(), flash_response_with_trigger(), is_system_route(), list_menu_items() (+18 more)

### Community 13 - "Community 13"
Cohesion: 0.11
Nodes (20): init(), refreshPulse(), addRecent(), canUseStorage(), collectPages(), init(), isPinned(), markLoaded() (+12 more)

### Community 14 - "Community 14"
Cohesion: 0.17
Nodes (13): compile_snapshot_includes_seeded_plugin_list_command_binding(), grant_role_policy(), normalize_non_empty(), plugin_binding_upsert_populates_compiled_snapshot(), plugin_cli_binding_policy_update_replaces_old_identity_binding(), plugin_http_binding_policy_update_replaces_old_identity_binding(), PolicyRepository, rejects_empty_policy_name() (+5 more)

### Community 15 - "Community 15"
Cohesion: 0.18
Nodes (26): activateTab(), bootstrapInitialPane(), canUseStorage(), closeTab(), createPane(), emitChange(), ensureDashboardTab(), ensurePane() (+18 more)

### Community 16 - "Community 16"
Cohesion: 0.09
Nodes (24): Admin Asset Bundles Declaration, Admin Dynamic Menu System, Admin Workspace Tabbed Navigation, DbGateway (Permissioned SQL), Database Permission Tiers (read/write/admin), Effective Permissions (Official Override), EventBus (Plugin Inter-Communication), HTMX Partial Content Loading (+16 more)

### Community 17 - "Community 17"
Cohesion: 0.14
Nodes (10): AdminPageEntry, AdminRegistry, AdminWidgetEntry, ApiRegistry, CliCommandEntry, CliRegistry, RouteEntry, test_admin_registry() (+2 more)

### Community 18 - "Community 18"
Cohesion: 0.13
Nodes (8): Authorizer, command_binding_presence_lookup_is_exact(), command_binding_requires_exact_name(), CompiledPolicySnapshot, HttpBinding, is_path_param(), path_pattern_matches(), split_path_segments()

### Community 19 - "Community 19"
Cohesion: 0.2
Nodes (17): api_http_bindings(), build_app(), parse_status_envelope(), plugin_api_dispatch(), PluginApiState, refresh_api_authorizer(), test_build_app_accepts_cookie_token_for_users_route(), test_build_app_allows_login_without_token() (+9 more)

### Community 20 - "Community 20"
Cohesion: 0.2
Nodes (15): CreateRoleForm, flash_response(), flash_response_with_trigger(), render_roles_rows(), role_permissions_form_partial(), role_permissions_update_partial(), roles_create_partial(), roles_delete_partial() (+7 more)

### Community 21 - "Community 21"
Cohesion: 0.15
Nodes (19): Admin Menu Design, Admin Menu Refactor Implementation Plan, Admin UI Redesign Design, Admin UI Redesign Implementation Plan, Admin Workspace Tabs + HTMX Partial Navigation Design, Admin Workspace Tabs Implementation Plan, Sushi Coding Standards, Engineering Standards Index (+11 more)

### Community 22 - "Community 22"
Cohesion: 0.18
Nodes (10): classify_keyword(), classify_sql(), DbGateway, DbGatewayError, DbPermission, first_statement_keyword(), has_multiple_statements(), is_token_char() (+2 more)

### Community 23 - "Community 23"
Cohesion: 0.21
Nodes (14): expandForRoute(), getChildren(), handleMenuClick(), hasActiveDescendant(), hasChildren(), init(), isActive(), loadMenu() (+6 more)

### Community 24 - "Community 24"
Cohesion: 0.25
Nodes (15): CreatePermissionForm, flash_response(), flash_response_with_trigger(), permissions_create_partial(), permissions_delete_partial(), permissions_table_partial(), permissions_update_partial(), render_permissions_rows() (+7 more)

### Community 25 - "Community 25"
Cohesion: 0.21
Nodes (7): active_log_service_cell(), bridge_layer_collects_warn_and_error_only(), current_log_service(), EventVisitor, layer(), LogServiceBridgeLayer, register_log_service()

### Community 26 - "Community 26"
Cohesion: 0.19
Nodes (5): custom_role_round_trip_uses_slug(), LoginRequest, TokenResponse, User, UserRole

### Community 27 - "Community 27"
Cohesion: 0.29
Nodes (7): allows_admin_when_command_binding_is_missing(), allows_command_when_role_has_grant(), denies_command_when_binding_exists_without_grant(), ensure_command_authorized(), ensure_command_authorized_with_authorizer(), normalize_role(), resolve_cli_role()

### Community 28 - "Community 28"
Cohesion: 0.25
Nodes (8): admin_auth_middleware(), AdminAuthContext, AdminAuthState, append_assets_to_html_response(), build_admin_router(), is_plugin_workspace_root_path(), is_valid_plugin_mount_id(), matches_static_prefix()

### Community 29 - "Community 29"
Cohesion: 0.35
Nodes (5): Claims, JwtService, test_create_and_verify_access_token(), test_invalid_token(), test_refresh_token_type()

### Community 30 - "Community 30"
Cohesion: 0.44
Nodes (5): SqliteStorage, test_run_migrations(), test_sqlite_execute_and_query(), test_sqlite_multiple_rows(), test_sqlite_null_handling()

### Community 31 - "Community 31"
Cohesion: 0.38
Nodes (8): CreateUserForm, flash_response(), flash_response_with_trigger(), render_users_rows(), users_create_partial(), users_delete_partial(), users_table_partial(), validate_create_user_form()

### Community 32 - "Community 32"
Cohesion: 0.27
Nodes (2): row_to_user(), UserRepository

### Community 33 - "Community 33"
Cohesion: 0.4
Nodes (6): load_template(), safe_join(), split_plugin_template_name(), TemplateService, validate_root(), validate_root_optional()

### Community 34 - "Community 34"
Cohesion: 0.47
Nodes (4): EventBus, test_emit_no_subscribers(), test_multiple_subscribers(), test_subscribe_and_emit()

### Community 35 - "Community 35"
Cohesion: 0.22
Nodes (4): create_user(), CreateUserRequest, PaginationParams, UsersRouteState

### Community 36 - "Community 36"
Cohesion: 0.33
Nodes (6): find_plugin(), plugin_pages_api(), plugin_workspace_context(), plugin_workspace_page(), PluginWorkspaceResponse, render_plugin_workspace_partial()

### Community 37 - "Community 37"
Cohesion: 0.36
Nodes (8): admin_can_access_admin_partials(), auth_state(), AuthState, AuthUser, extract_token_from_cookie(), non_admin_cannot_access_admin_partials(), require_auth(), viewer_without_policy_grant_is_denied_api_route()

### Community 38 - "Community 38"
Cohesion: 0.22
Nodes (5): sqlite_to_json(), Storage, StorageConn, StorageConn<'a>, StorageError

### Community 39 - "Community 39"
Cohesion: 0.25
Nodes (2): LogEntry, LogService

### Community 40 - "Community 40"
Cohesion: 0.46
Nodes (7): cms_category_delete_conflicts_when_posts_exist(), cms_cli_dispatch_supports_page_list(), cms_post_list_category_query_filters_rows(), cms_public_page_route_hides_draft_content(), cms_public_post_detail_hides_draft_posts(), cms_soft_deleted_posts_are_hidden_from_list(), repo_root()

### Community 41 - "Community 41"
Cohesion: 0.25
Nodes (0): 

### Community 42 - "Community 42"
Cohesion: 0.46
Nodes (7): create_sandboxed_vm(), test_sandbox_allows_basic_lua(), test_sandbox_allows_string_ops(), test_sandbox_allows_tables(), test_sandbox_blocks_io(), test_sandbox_blocks_os_execute(), test_sandbox_blocks_require()

### Community 43 - "Community 43"
Cohesion: 0.25
Nodes (1): M.new()

### Community 44 - "Community 44"
Cohesion: 0.29
Nodes (2): AuthRouteState, RefreshRequest

### Community 45 - "Community 45"
Cohesion: 0.67
Nodes (5): bootstrap(), hydrate_authorizer_snapshot(), resolve_dir(), resolve_static_dir(), resolve_templates_dir()

### Community 46 - "Community 46"
Cohesion: 0.47
Nodes (4): module_template(), module_to_admin_path(), workspace_partial(), WorkspaceAssetsResponse

### Community 47 - "Community 47"
Cohesion: 0.47
Nodes (4): login_error_response(), login_submit(), LoginForm, render_login_flash_html()

### Community 48 - "Community 48"
Cohesion: 0.6
Nodes (5): install_plugin_require(), require_loads_plugin_local_module(), require_rejects_parent_traversal(), safe_module_join(), validate_module_name()

### Community 49 - "Community 49"
Cohesion: 0.7
Nodes (4): merge_static_prefix(), normalize_static_url_prefix(), render_template(), render_template_with_context()

### Community 50 - "Community 50"
Cohesion: 0.4
Nodes (0): 

### Community 51 - "Community 51"
Cohesion: 0.5
Nodes (1): SushiContext

### Community 52 - "Community 52"
Cohesion: 0.4
Nodes (1): M.new()

### Community 53 - "Community 53"
Cohesion: 0.5
Nodes (3): M.new(), parse_urlencoded(), url_decode()

### Community 54 - "Community 54"
Cohesion: 0.5
Nodes (2): ConfigArgs, ConfigCommand

### Community 55 - "Community 55"
Cohesion: 0.5
Nodes (2): PluginArgs, PluginCommand

### Community 56 - "Community 56"
Cohesion: 0.5
Nodes (2): Cli, Commands

### Community 57 - "Community 57"
Cohesion: 0.5
Nodes (1): sushi.init()

### Community 58 - "Community 58"
Cohesion: 0.67
Nodes (2): M.execute(), M.query()

### Community 59 - "Community 59"
Cohesion: 0.5
Nodes (4): Admin Panel, Alpine.js Framework, HTMX Framework, TailwindCSS Framework

### Community 60 - "Community 60"
Cohesion: 0.67
Nodes (1): ServeArgs

### Community 61 - "Community 61"
Cohesion: 0.67
Nodes (1): RunArgs

### Community 62 - "Community 62"
Cohesion: 0.67
Nodes (1): SeedArgs

### Community 63 - "Community 63"
Cohesion: 0.67
Nodes (0): 

### Community 64 - "Community 64"
Cohesion: 0.67
Nodes (0): 

### Community 65 - "Community 65"
Cohesion: 0.67
Nodes (1): M.register()

### Community 66 - "Community 66"
Cohesion: 0.67
Nodes (0): 

### Community 67 - "Community 67"
Cohesion: 1.0
Nodes (2): M.parse_urlencoded(), url_decode()

### Community 68 - "Community 68"
Cohesion: 0.67
Nodes (0): 

### Community 69 - "Community 69"
Cohesion: 1.0
Nodes (2): escape_html(), M.to_html()

### Community 70 - "Community 70"
Cohesion: 0.67
Nodes (0): 

### Community 71 - "Community 71"
Cohesion: 0.67
Nodes (3): Admin Workspace RBAC Mapping, RBAC Permission Model, Workspace Navigation (HTMX + Tabs)

### Community 72 - "Community 72"
Cohesion: 1.0
Nodes (0): 

### Community 73 - "Community 73"
Cohesion: 1.0
Nodes (1): TemplateError

### Community 74 - "Community 74"
Cohesion: 1.0
Nodes (0): 

### Community 75 - "Community 75"
Cohesion: 1.0
Nodes (0): 

### Community 76 - "Community 76"
Cohesion: 1.0
Nodes (0): 

### Community 77 - "Community 77"
Cohesion: 1.0
Nodes (0): 

### Community 78 - "Community 78"
Cohesion: 1.0
Nodes (0): 

### Community 79 - "Community 79"
Cohesion: 1.0
Nodes (2): Su Shi (苏轼) - Project Namesake, Sushi Platform

### Community 80 - "Community 80"
Cohesion: 1.0
Nodes (2): Database Migration Tables, RBAC Data Model (SQL)

### Community 81 - "Community 81"
Cohesion: 1.0
Nodes (2): Sushi Favicon Su Shi Minimal Ink Design, Su Shi Minimal Ink Favicon Implementation Plan

### Community 82 - "Community 82"
Cohesion: 1.0
Nodes (2): JWT Authentication (access + refresh tokens), RBAC (Role-Based Access Control)

### Community 83 - "Community 83"
Cohesion: 1.0
Nodes (2): Su Shi Minimal Ink Mark (Poetry Abstract), Favicon SVG (Su Shi Minimal Ink Mark)

### Community 84 - "Community 84"
Cohesion: 1.0
Nodes (0): 

### Community 85 - "Community 85"
Cohesion: 1.0
Nodes (0): 

### Community 86 - "Community 86"
Cohesion: 1.0
Nodes (0): 

### Community 87 - "Community 87"
Cohesion: 1.0
Nodes (0): 

### Community 88 - "Community 88"
Cohesion: 1.0
Nodes (0): 

### Community 89 - "Community 89"
Cohesion: 1.0
Nodes (0): 

### Community 90 - "Community 90"
Cohesion: 1.0
Nodes (0): 

### Community 91 - "Community 91"
Cohesion: 1.0
Nodes (0): 

### Community 92 - "Community 92"
Cohesion: 1.0
Nodes (0): 

### Community 93 - "Community 93"
Cohesion: 1.0
Nodes (0): 

### Community 94 - "Community 94"
Cohesion: 1.0
Nodes (0): 

## Knowledge Gaps
- **110 isolated node(s):** `ServeArgs`, `ConfigArgs`, `ConfigCommand`, `RunArgs`, `PluginArgs` (+105 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **Thin community `Community 72`** (2 nodes): `dashboard.rs`, `dashboard_page()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 73`** (2 nodes): `template_error.rs`, `TemplateError`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 74`** (2 nodes): `M.escape()`, `html.lua`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 75`** (2 nodes): `slug.lua`, `M.normalize()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 76`** (2 nodes): `M.new()`, `category.lua`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 77`** (2 nodes): `post.lua`, `M.new()`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 78`** (2 nodes): `M.new()`, `page.lua`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 79`** (2 nodes): `Su Shi (苏轼) - Project Namesake`, `Sushi Platform`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 80`** (2 nodes): `Database Migration Tables`, `RBAC Data Model (SQL)`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 81`** (2 nodes): `Sushi Favicon Su Shi Minimal Ink Design`, `Su Shi Minimal Ink Favicon Implementation Plan`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 82`** (2 nodes): `JWT Authentication (access + refresh tokens)`, `RBAC (Role-Based Access Control)`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 83`** (2 nodes): `Su Shi Minimal Ink Mark (Poetry Abstract)`, `Favicon SVG (Su Shi Minimal Ink Mark)`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 84`** (1 nodes): `lib.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 85`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 86`** (1 nodes): `lib.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 87`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 88`** (1 nodes): `lib.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 89`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 90`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 91`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 92`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 93`** (1 nodes): `lib.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.
- **Thin community `Community 94`** (1 nodes): `mod.rs`
  Too small to be a meaningful cluster - may be noise or needs more connections extracted.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `add()` connect `Community 2` to `Community 0`, `Community 1`, `Community 3`, `Community 15`, `Community 23`?**
  _High betweenness centrality (0.045) - this node is a cross-community bridge._
- **Why does `dismissToast()` connect `Community 1` to `Community 2`?**
  _High betweenness centrality (0.026) - this node is a cross-community bridge._
- **Why does `ae()` connect `Community 0` to `Community 1`?**
  _High betweenness centrality (0.022) - this node is a cross-community bridge._
- **Are the 2 inferred relationships involving `He()` (e.g. with `set()` and `keys()`) actually correct?**
  _`He()` has 2 INFERRED edges - model-reasoned connections that need verification._
- **What connects `ServeArgs`, `ConfigArgs`, `ConfigCommand` to the rest of the system?**
  _110 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Community 0` be split into smaller, more focused modules?**
  _Cohesion score 0.03 - nodes in this community are weakly interconnected._
- **Should `Community 1` be split into smaller, more focused modules?**
  _Cohesion score 0.02 - nodes in this community are weakly interconnected._