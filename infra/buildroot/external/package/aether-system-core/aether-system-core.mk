AETHER_SYSTEM_CORE_VERSION = 0.1.0
AETHER_SYSTEM_CORE_SITE = $(abspath $(BR2_EXTERNAL_AETHER_PATH)/../../..)
AETHER_SYSTEM_CORE_SITE_METHOD = local
AETHER_SYSTEM_CORE_LICENSE = MIT
AETHER_SYSTEM_CORE_DEPENDENCIES = host-rustc
AETHER_SYSTEM_CORE_OVERRIDE_SRCDIR_RSYNC_EXCLUSIONS = \
	--exclude=/artifacts \
	--exclude=/build \
	--exclude=/dist \
	--exclude=/out \
	--exclude=/target \
	--exclude=/.git \
	--exclude=__pycache__

define AETHER_SYSTEM_CORE_BUILD_CMDS
	cd $(@D) && \
		$(TARGET_MAKE_ENV) \
		$(TARGET_CONFIGURE_OPTS) \
		CARGO_HOME="$(DL_DIR)/br-cargo-home" \
		CARGO_TARGET_DIR="$(@D)/target" \
		CARGO_BUILD_TARGET="$(RUSTC_TARGET_NAME)" \
		CARGO_TARGET_$(call UPPERCASE,$(RUSTC_TARGET_NAME))_LINKER=$(notdir $(TARGET_CROSS))gcc \
		cargo build --release --manifest-path Cargo.toml -p aether-system-core -p aetherctl -p aether-storage
endef

define AETHER_SYSTEM_CORE_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 $(@D)/target/$(RUSTC_TARGET_NAME)/release/aether-system-core \
		$(TARGET_DIR)/usr/sbin/aether-system-core
	$(INSTALL) -D -m 0755 $(@D)/target/$(RUSTC_TARGET_NAME)/release/aetherctl \
		$(TARGET_DIR)/usr/bin/aetherctl
	$(INSTALL) -D -m 0755 $(@D)/target/$(RUSTC_TARGET_NAME)/release/aether-filesystemd \
		$(TARGET_DIR)/usr/sbin/aether-filesystemd
endef

$(eval $(generic-package))
