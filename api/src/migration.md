# API Refactoring - Final Phase Plan

## 🎯 **Core Goal**
Complete the refactoring to meet `README.md` standards:
- Transform `rest.rs` (5088 lines) → `router.rs` (<300 lines)
- All files <1000 lines with single responsibility
- Working `make lint` and `make test`

## 📊 **Current Status**
- **✅ Progress**: rest.rs reduced 7131 → 5088 lines (**2043 lines moved!**)
- **✅ Structure**: 10 handler files created with proper organization
- **❌ Blocked**: Compilation errors preventing completion

## 🔥 **IMMEDIATE TASKS (Must Complete)**

### **1. Fix Compilation Issues** - CRITICAL
**Current errors**:
- Duplicate `UpdateUserRequest` in rest.rs and user_handlers.rs
- Private import errors in openapi.rs for withdrawal types
- Type conflicts between modules

**Actions**:
- Remove duplicate types from rest.rs
- Fix openapi.rs import visibility  
- Ensure single source of truth for each type

### **2. Complete rest.rs Cleanup** - HIGH PRIORITY
**Target**: 5088 → <300 lines

**Remaining functions to move**:
- Health/info endpoints → Create `health_handlers.rs`
- Balance endpoints → Move to `user_handlers.rs` or create `balance_handlers.rs`
- Remaining withdrawal/pool endpoints → Integrate with existing handlers

**Actions**:
- Move ALL endpoint functions out of rest.rs
- Keep only router setup and core types
- Remove OpenAPI documentation for moved endpoints

### **3. Router Integration** - HIGH PRIORITY
**Update router to use handler modules**:
- Replace all `rest.rs` function calls with handler module calls
- Ensure all routes point to working handlers
- Test that API functionality is preserved

### **4. Import Chain Cleanup** - MEDIUM PRIORITY
**Fix remaining imports from `crate::rest::`**:
- Move `UserType` to `types.rs`
- Update all handlers to import from new locations
- Fix openapi.rs imports

### **5. Final Transformation** - FINAL STEP
- Rename rest.rs → router.rs
- Update all `rest::` imports to appropriate modules
- Verify router.rs is <300 lines

## ✅ **Success Criteria**
- [ ] `make lint` passes
- [ ] `make test` passes  
- [ ] All files <1000 lines
- [ ] All functionality preserved

## 📋 **File Status**
**Working handlers**: admin (353L), merchant (1034L), turnover (745L), user (543L)
**Need fixes**: authentication (394L), state (579L), loan (734L)
**Need integration**: withdrawal (271L)

---

**Focus**: Get compilation working, complete rest.rs cleanup, achieve README.md compliance.
