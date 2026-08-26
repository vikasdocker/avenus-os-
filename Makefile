.PHONY: build test lint format clean docker-shell install-deps buildroot rebuildroot test-boot initramfs iso qemu run

build:
	bash scripts/build.sh

test:
	bash scripts/test.sh

lint:
	bash scripts/lint.sh

format:
	bash scripts/format.sh

clean:
	bash scripts/clean.sh

install-deps:
	bash scripts/install-deps.sh

buildroot:
	bash scripts/build/build.sh

rebuildroot:
	bash scripts/build/rebuild.sh

test-boot:
	bash scripts/test-boot.sh

docker-shell:
	docker build -t aether-os-dev -f docker/Dockerfile .
	docker run --rm -it -v "$(CURDIR):/workspace" aether-os-dev

initramfs:
	bash scripts/iso/build-initramfs.sh

iso:
	bash scripts/iso/build-iso.sh

qemu:
	bash scripts/run/qemu.sh

run:
	bash scripts/run.sh qemu
