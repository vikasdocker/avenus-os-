AETHER_CORE_VERSION = 0.1.0
AETHER_CORE_SITE = $(BR2_EXTERNAL_AETHER_PATH)/package/aether-core/src
AETHER_CORE_SITE_METHOD = local
AETHER_CORE_LICENSE = MIT

define AETHER_CORE_INSTALL_TARGET_CMDS
	$(INSTALL) -D -m 0755 $(@D)/aether-core $(TARGET_DIR)/usr/sbin/aether-core
endef

$(eval $(generic-package))

