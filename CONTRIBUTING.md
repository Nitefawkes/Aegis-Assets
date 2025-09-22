# Contributing to Aegis-Assets

Thank you for your interest in contributing to Aegis-Assets! This guide will help you understand our development process, coding standards, and—most importantly—our compliance-first approach.

## 🛡️ Compliance-First Development

Aegis-Assets is built on a **compliance-first architecture**. This means every contribution must consider legal implications and maintain our commitment to responsible asset extraction.

### Before You Contribute

1. **Read the Compliance Manifesto**: Understand why we exclude certain publishers and formats
2. **Review Existing Profiles**: Check `compliance-profiles/` directory for publisher precedents  
3. **Consider Legal Impact**: Ask yourself: "Does this contribution respect IP rights?"

### Plugin Development Guidelines

#### ✅ Acceptable Contributions

- **Permissive Publishers**: Bethesda, Valve, CD Projekt Red formats
- **Neutral Publishers**: Most indie/Unity games with proper disclaimers
- **Performance Improvements**: Faster extraction, better memory usage
- **Quality of Life**: Better UX, documentation, tooling

#### ❌ Unacceptable Contributions

- **High-Risk Publishers**: Nintendo, Take-Two formats in official capacity
- **DRM Circumvention**: Code that bypasses encryption or protection
- **Asset Redistribution**: Tools that enable direct asset sharing
- **Compliance Bypass**: Features that skip or ignore compliance checks

#### 🔍 Gray Area Guidelines

For unclear cases, follow this decision tree:

1. **Does the publisher explicitly support modding?** → ✅ Likely acceptable
2. **History of aggressive IP enforcement?** → ❌ Avoid official support  
3. **Large community of modders?** → 🔍 Community plugin with disclaimers
4. **Educational/preservation use case?** → 🔍 Discuss with maintainers first

## 🏗️ Development Setup

### Prerequisites

- **Rust 1.70+**: Latest stable toolchain
- **Python 3.8+**: Optional (bindings are experimental and currently stubbed)
- **Git**: Version control
- **IDE**: VSCode with rust-analyzer recommended

### Getting Started

```bash
# Clone the repository
git clone https://github.com/aegis-assets/aegis-assets.git
cd aegis-assets

# Build the workspace
cargo build

# Run tests
cargo test

# Check compliance profiles
cargo run --bin compliance-check -- --profiles compliance-profiles/

# Build Python bindings (optional)
cd aegis-python
pip install maturin
maturin develop
```

### Repository Structure

```
aegis-assets/
├── aegis-core/              # Core Rust library
├── aegis-python/            # Python bindings
├── aegis-plugins/           # Format plugins
│   ├── unity/
│   ├── unreal/
│   └── plugin-template/
├── compliance-profiles/     # Publisher compliance data
├── docs/                    # Documentation
└── examples/               # Usage examples
```

## 🔌 Plugin Development

### Creating a New Plugin

1. **Use the template**:
   ```bash
   cargo generate --git https://github.com/aegis-assets/plugin-template
   # Or copy aegis-plugins/plugin-template/
   ```

2. **Implement required traits**:
   ```rust
   use aegis_core::{ArchiveHandler, PluginFactory};

   pub struct MyFormatHandler;

   impl ArchiveHandler for MyFormatHandler {
       fn detect(bytes: &[u8]) -> bool { /* ... */ }
       fn open(path: &Path) -> Result<Self> { /* ... */ }
       // ... other required methods
   }
   ```

3. **Add compliance checking**:
   ```rust
   impl MyFormatHandler {
       fn open(path: &Path) -> Result<Self> {
           let profile = Self::load_compliance_profile();
           
           // Check if extraction is allowed
           if profile.enforcement_level == ComplianceLevel::HighRisk {
               return Err(ComplianceError::HighRiskPublisher);
           }
           
           // ... continue with extraction
       }
   }
   ```

4. **Create compliance profile** (if new publisher):
   ```yaml
   # compliance-profiles/new-publisher.yaml
   publisher: "New Publisher"
   enforcement_level: "Neutral"  # Start conservative
   official_support: false
   bounty_eligible: false
   enterprise_warning: "New publisher - compliance status under review"
   
   supported_formats:
     newformat: "CommunityOnly"
   ```

### Plugin Review Process

1. **Technical Review**: Code quality, performance, safety
2. **Compliance Review**: Legal risk assessment, publisher research
3. **Community Feedback**: Discussion period for controversial additions
4. **Maintainer Approval**: Final approval by core team

## 🧪 Testing

### Test Categories

- **Unit Tests**: Individual function testing
- **Integration Tests**: Plugin + core interaction  
- **Compliance Tests**: Verify legal checks work correctly
- **Performance Tests**: Benchmark extraction speed

### Writing Compliance Tests

```rust
#[test]
fn test_high_risk_publisher_blocked() {
    let handler = MyFormatHandler::open("nintendo_game.sarc");
    
    // Should fail with compliance error in strict mode
    assert!(matches!(handler.unwrap_err(), ComplianceError::HighRiskPublisher));
}

#[test]
fn test_permissive_publisher_allowed() {
    let handler = BethesdaHandler::open("skyrim.bsa");
    
    // Should succeed with appropriate warnings
    assert!(handler.is_ok());
    assert!(handler.unwrap().compliance_profile().official_support);
}
```

## 📝 Code Style

### Rust Guidelines

- **Format**: Use `rustfmt` with default settings
- **Linting**: Pass `clippy` with no warnings
- **Documentation**: Public APIs must have doc comments
- **Error Handling**: Use `anyhow::Result` for error propagation
- **Logging**: Use `tracing` crate for structured logging

### Commit Message Format

```
type(scope): description

[optional body]

[optional footer]
```

Types: `feat`, `fix`, `docs`, `test`, `refactor`, `compliance`

Examples:
- `feat(unity): add Unity 2023 support`
- `compliance(nintendo): update risk assessment`
- `fix(export): handle empty texture data`

## 🚀 Pull Request Process

### Checklist

- [ ] **Compliance Review**: No high-risk publisher support added
- [ ] **Tests Pass**: All existing tests continue to pass
- [ ] **New Tests**: Added tests for new functionality
- [ ] **Documentation**: Updated relevant docs
- [ ] **Performance**: No significant performance regression
- [ ] **Code Quality**: Passes clippy and rustfmt

### Review Timeline

- **Initial Response**: 48 hours for triage
- **Technical Review**: 3-5 days for code review
- **Compliance Review**: Additional 2-3 days for legal assessment
- **Community Feedback**: 7 days for controversial changes

### Merge Criteria

1. **2 Approvals**: Technical reviewer + compliance reviewer
2. **All Tests Pass**: CI must be green
3. **No Legal Red Flags**: Compliance reviewer approval
4. **Documentation Updated**: Changes reflected in docs

## 🏆 Recognition

### Contribution Types

- **Code Contributions**: Plugin development, bug fixes, features
- **Compliance Research**: Publisher policy research, legal analysis  
- **Documentation**: Guides, examples, API documentation
- **Community Support**: Helping users, answering questions

### Recognition Levels

- **Contributors**: Listed in repository and releases
- **Plugin Pioneers**: Highlighted for significant format support additions
- **Compliance Advocates**: Recognized for legal research and guidance
- **Core Team**: Invited to join core development team

## 📞 Getting Help

### Communication Channels

- **Discord**: Real-time discussion and support
- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: Design discussions and questions
- **Email**: compliance@aegis-assets.org for legal questions

### Mentorship Program

New contributors are paired with experienced developers for:
- Architecture guidance
- Compliance framework understanding
- Code review and feedback
- Career development in open source

## 🎯 Contribution Priorities

### High Priority

1. **Unity 2023+ Support**: Latest Unity formats
2. **Performance Optimization**: Memory usage and speed improvements
3. **Enterprise Features**: Audit logging, compliance dashboards
4. **Documentation**: User guides and API documentation

### Medium Priority  

1. **Source Engine Updates**: Source 2 format support
2. **AI Integration**: Automated tagging and classification
3. **GUI Improvements**: Better user experience
4. **Plugin Marketplace**: Community plugin discovery

### Low Priority (Community Driven)

1. **Obscure Formats**: Niche game engines
2. **Legacy Support**: Very old game versions
3. **Platform-Specific**: Console-only formats

---

**By contributing to Aegis-Assets, you're helping build the future of responsible asset extraction. Together, we're creating professional-grade infrastructure that respects both creator rights and community needs.**

## Quick Start for New Contributors

1. **Join Discord** to introduce yourself
2. **Read the Compliance Manifesto** in `docs/COMPLIANCE.md`
3. **Pick a "good first issue"** labeled issue
4. **Ask questions** - we're here to help!
5. **Submit your first PR** following the guidelines above

Welcome to the team! 🎉
