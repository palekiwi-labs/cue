# NixOS module for the acuity service.
#
# Consumed via the flake's `nixosModules.acuity` output. The flake passes its
# own `self` reference into this module so the default `package` option can
# reference the workspace build without any consumer wiring.
#
# acuity is env-driven (no CLI flags), so service configuration is injected via
# `environment` and `EnvironmentFile`, not ExecStart arguments.
#
# ACUITY_GOTIFY_TOKEN is optional. If unset, Gotify notifications are disabled
# and the service starts normally. Supply it via `environmentFile` if needed.
self:
{ config, lib, pkgs, ... }:

let
  cfg = config.services.acuity;
in
{
  options.services.acuity = {
    enable = lib.mkEnableOption "acuity observability ingestion server";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.system}.acuity;
      defaultText = lib.literalExpression "self.packages.\${pkgs.system}.acuity";
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

    dataDir = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib";
      description = ''
        Parent directory passed to acuity as ACUITY_DATA_DIR. The binary
        appends `acuity/events.db` to produce the final database path, so
        the default `/var/lib` results in `/var/lib/acuity/events.db`.

        The `/var/lib/acuity` subdirectory is created and owned by systemd
        via `StateDirectory = "acuity"` when the default is used. Override
        only if you need the database elsewhere; you are then responsible for
        ensuring the directory exists and is writable by the service user.
      '';
    };

    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      description = ''
        Optional path to a systemd EnvironmentFile that supplies additional
        environment variables, most commonly:

            ACUITY_GOTIFY_TOKEN=<your-gotify-app-token>

        When this token is present, acuity forwards SessionIdle events to
        Gotify. When absent (or when this option is null), notifications are
        simply disabled — the service still starts and persists events.

        Do NOT prefix the path with `-` if you want the service to fail
        loudly when the file is absent.
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
      description = "acuity -- observability ingestion server (cue ecosystem)";
      wantedBy = [ "multi-user.target" ];
      after = [ "network.target" ];

      serviceConfig = {
        Type = "exec";
        User = cfg.user;
        Group = cfg.group;
        ExecStart = "${cfg.package}/bin/acuity";
        Restart = "on-failure";
        RestartSec = "5s";

        # Persistent state: systemd creates /var/lib/acuity and grants
        # ownership to cfg.user. Only active when dataDir is the default
        # (/var/lib); for custom paths the operator manages the directory.
        StateDirectory = lib.mkIf (cfg.dataDir == "/var/lib") "acuity";

        # Optional environment file (supplies ACUITY_GOTIFY_TOKEN etc.).
        EnvironmentFile = lib.mkIf (cfg.environmentFile != null)
          cfg.environmentFile;

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

        # Allow writes to the data directory when using a custom path
        # (StateDirectory only covers /var/lib).
        ReadWritePaths = lib.mkIf (cfg.dataDir != "/var/lib")
          [ cfg.dataDir ];
      };

      environment = {
        ACUITY_GOTIFY_URL = cfg.gotifyUrl;
        ACUITY_PORT = toString cfg.port;
        # Tell the binary where to put events.db.
        ACUITY_DATA_DIR = cfg.dataDir;
        RUST_LOG = "info";
      };
    };
  };
}
