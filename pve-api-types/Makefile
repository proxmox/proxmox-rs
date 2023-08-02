OUTPUT_DIR := src/generated

GENERATOR_SOURCES != find generator-lib

$(OUTPUT_DIR)/code.rs: $(OUTPUT_DIR)/types.rs
$(OUTPUT_DIR)/types.rs: generate.pl $(GENERATOR_SOURCES)
	rm -rf $(OUTPUT_DIR)
	mkdir $(OUTPUT_DIR)
	perl ./generate.pl $(OUTPUT_DIR)

.PHONY: clean
clean:
	@true
