OUTPUT_DIR := src/generated

GENERATOR_SOURCES != find generator-lib

all: $(OUTPUT_DIR)/code.rs $(OUTPUT_DIR)/types.rs

pve-api.json: dump_api.pl generator-lib/ApiDump.pm
	perl dump_api.pl >$@.tmp
	mv $@.tmp $@

$(OUTPUT_DIR)/code.rs $(OUTPUT_DIR)/types.rs &: generate.pl $(GENERATOR_SOURCES)
	rm -rf $(OUTPUT_DIR)
	mkdir $(OUTPUT_DIR)
	perl ./generate.pl $(OUTPUT_DIR)

.PHONY: refresh
refresh:
	rm -f $(OUTPUT_DIR)/code.rs
	rm -f $(OUTPUT_DIR)/types.rs
	$(MAKE) $(OUTPUT_DIR)/code.rs

.PHONY: clean
clean:
	@true
