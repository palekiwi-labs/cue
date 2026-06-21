# NixOS module for the acuity service.
#
# Consumed via the flake's `nixosModules.default` (also aliased as
# `nixosModules.acuity`) output. The flake passes its own `self` reference into
# this module so the default `package` option can reference the workspace build
# without any consumer wiring.
#
# acuity is env-driven (no CLI flags), so service configuration is injected via
# `environment` and `EnvironmentFile`, not ExecStart arguments.
#
# The EnvironmentFile must contain:
#     ACUITY_GOTIFY_TOKEN=<your-gotify-app-token>
# This matches the read at crates/acuity/src/main.rs:48.
self:
{ config, lib, pkgs, ... }:

let
  cfg = config.services.acuity;
in
{
  options.services.acuity = {
    enable = lib.mkEnableOption "acuity session.idle observability server";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.system}.default;
      defaultText = lib.literalExpression "self.packages.\${pkgs.system}.default";
      description = "acuity package to run. Defaults to the workspace build.";
    };

    gotifyUrl = lib.mkOption {
      type = lib.types.str;
      default = "http://localhost";
      description = "Base URL of the Gotify server (no trailing slash).";
    };

    port = lib.mkOption {
      type = lib.types.port;
      default = 33222;
      description = "Port for acuity to listen on.";
    };

    environmentFile = lib.mkOption {
      type = lib.types.path;
      description = ''
        Path to an environment file (systemd EnvironmentFile format) that
        provides at least:

            ACUITY_GOTIFY_TOKEN=<your-gotify-app-token>

        The token is read directly by the binary at startup; missing it causes
        a hard exit. Do NOT prefix this option with `-` if you want the service
        to fail loudly when the file is absent.
      '';
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "acuity";
      description = "User to run the service as.";
    };

    group = lib.mkOption {
      type = lib.types.str;
      default = "acuity";
      description = "Group to run the service as.";
    };
  };

  config = lib.mkIf cfg.enable {
    users.users.${cfg.user} = lib.mkIf (cfg.user == "acuity") {
      isSystemUser = true;
      group = cfg.group;
      description = "acuity service user";
    };

    users.groups.${cfg.group} = lib.mkIf (cfg.group == "acuity") { };

    systemd.services.acuity = {
      description = "acuity -- session.idle observability server (cue ecosystem)";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];

      serviceConfig = {
        Type = "exec";
        User = cfg.user;
        Group = cfg.group;
        ExecStart = "${cfg.package}/bin/acuity";
        Restart = "on-failure";
        RestartSec = "5s";

        # Token read by the binary as ACUITY_GOTIFY_TOKEN (main.rs:48).
        EnvironmentFile = cfg.environmentFile;

        # Hardening. Safe because the listen port is >1024 and the only
        # network requirement is outbound HTTPS to Gotify.
        NoNewPrivileges = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        PrivateTmp = true;
        PrivateDevices = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        RestrictAddressFamilies = [ "AF_INET" "AF_INET6" ];
        CapabilityBoundingSet = "";
        RestrictNamespaces = true;
        RestrictRealtime = true;
        MemoryDenyWriteExecute = true;
        LockPersonality = true;
      };

      environment = {
        ACUITY_GOTIFY_URL = cfg.gotifyUrl;
        ACUITY_PORT = toString cfg.port;
        RUST_LOG = "info";
      };
    };
  };
}
