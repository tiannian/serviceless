# Specs Directory Guide

## Overview

The `specs` directory is used to record and maintain all development standards and specifications for this codebase. All specification files are written in Markdown format for easy reading and maintenance.

## File Naming Convention

- All files must be named using the `NN-filename.md` format
- `NN` is a two-digit number (00-99) used to specify file priority and reading order
- `filename` is a descriptive English filename using lowercase letters and hyphens
- File extension must be `.md`

### Examples

- `00-specs-guide.md` - This file, describing the basic specifications for the specs directory
- `01-coding-standards.md` - Coding standards
- `02-architecture.md` - Architecture design specifications
- `10-testing.md` - Testing specifications

## Content Requirements

Each specification file should include:

1. **Title**: Clear and descriptive title
2. **Overview**: Scope and purpose of the specification
3. **Detailed Specifications**: Specific rules and requirements
4. **Examples**: Appropriate code or configuration examples
5. **References**: Links to related documentation or resources (if applicable)

## Maintenance Principles

- All specification files should be kept up to date to reflect the current state of the codebase
- When specifications change, relevant files should be updated promptly
- When adding new specifications, choose an appropriate number and create a new file
- Deleted or deprecated specifications should be clearly marked in the file

## Language Requirements

- All specification files must be written in English only
- Use clear and concise English prose
- Technical terms should be used consistently throughout the documentation
