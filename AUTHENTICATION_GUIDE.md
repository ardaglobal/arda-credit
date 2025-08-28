# Authentication Integration Guide

This guide shows how to integrate with the Arda Credit API authentication system from your frontend application.

## Overview

The Arda Credit API uses JWT (JSON Web Token) authentication with the following flow:
1. **Registration**: Create user account with email and password
2. **Login**: Exchange email/password for JWT token
3. **Authenticated Requests**: Include JWT token in requests
4. **Token Management**: Handle token expiry and refresh

## API Endpoints

### Authentication Endpoints
- `POST /api/auth/login` - Login with email/password
- `POST /api/auth/logout` - Logout (for logging purposes)
- `POST /api/ccr/user` - Register new user

### Protected Endpoints
All other endpoints require authentication except:
- `/health` - Health check
- `/` - API info
- `/api-docs` - OpenAPI documentation

## Frontend Integration Examples

### JavaScript/TypeScript Integration

#### 1. Authentication Service Class

```javascript
class AuthService {
    constructor(apiBaseUrl = '/api') {
        this.apiBaseUrl = apiBaseUrl;
        this.tokenKey = 'arda_access_token';
        this.userKey = 'arda_user_data';
    }

    // Register new user
    async register(email, password, userType = 'investor') {
        const response = await fetch(`${this.apiBaseUrl}/ccr/user`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                email,
                password,
                user_type: userType,
            }),
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || 'Registration failed');
        }

        return await response.json();
    }

    // Login user
    async login(email, password) {
        const response = await fetch(`${this.apiBaseUrl}/auth/login`, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({ email, password }),
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || 'Login failed');
        }

        const data = await response.json();
        
        // Store token and user data
        localStorage.setItem(this.tokenKey, data.access_token);
        localStorage.setItem(this.userKey, JSON.stringify(data.user));
        
        return data;
    }

    // Logout user
    async logout() {
        const token = this.getToken();
        
        if (token) {
            try {
                await fetch(`${this.apiBaseUrl}/auth/logout`, {
                    method: 'POST',
                    headers: {
                        'Content-Type': 'application/json',
                        'Authorization': `Bearer ${token}`,
                    },
                    body: JSON.stringify({}),
                });
            } catch (error) {
                console.warn('Logout request failed:', error);
            }
        }

        // Clear local storage
        localStorage.removeItem(this.tokenKey);
        localStorage.removeItem(this.userKey);
    }

    // Get stored token
    getToken() {
        return localStorage.getItem(this.tokenKey);
    }

    // Get stored user data
    getUser() {
        const userData = localStorage.getItem(this.userKey);
        return userData ? JSON.parse(userData) : null;
    }

    // Check if user is authenticated
    isAuthenticated() {
        const token = this.getToken();
        if (!token) return false;

        try {
            // Decode JWT to check expiry (basic check)
            const payload = JSON.parse(atob(token.split('.')[1]));
            const now = Math.floor(Date.now() / 1000);
            return payload.exp > now;
        } catch {
            return false;
        }
    }

    // Make authenticated API request
    async authenticatedFetch(url, options = {}) {
        const token = this.getToken();
        
        if (!token) {
            throw new Error('No authentication token available');
        }

        const defaultOptions = {
            headers: {
                'Content-Type': 'application/json',
                'Authorization': `Bearer ${token}`,
                ...options.headers,
            },
        };

        const response = await fetch(url, { ...options, ...defaultOptions });

        // Handle token expiry
        if (response.status === 401) {
            this.logout();
            throw new Error('Authentication expired. Please login again.');
        }

        return response;
    }
}

// Create global instance
const authService = new AuthService();
export default authService;
```

#### 2. React Hook for Authentication

```jsx
import { useState, useEffect, useContext, createContext } from 'react';
import authService from './authService';

// Auth Context
const AuthContext = createContext(null);

export const AuthProvider = ({ children }) => {
    const [user, setUser] = useState(null);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        // Check for existing authentication on app load
        if (authService.isAuthenticated()) {
            setUser(authService.getUser());
        }
        setLoading(false);
    }, []);

    const register = async (email, password, userType) => {
        try {
            await authService.register(email, password, userType);
            // After registration, user needs to login
            return { success: true, requiresLogin: true };
        } catch (error) {
            throw error;
        }
    };

    const login = async (email, password) => {
        try {
            const result = await authService.login(email, password);
            setUser(result.user);
            return result;
        } catch (error) {
            throw error;
        }
    };

    const logout = async () => {
        try {
            await authService.logout();
            setUser(null);
        } catch (error) {
            console.error('Logout error:', error);
            // Still clear local state even if API call fails
            setUser(null);
        }
    };

    const value = {
        user,
        login,
        logout,
        register,
        isAuthenticated: authService.isAuthenticated(),
        loading,
    };

    return (
        <AuthContext.Provider value={value}>
            {children}
        </AuthContext.Provider>
    );
};

export const useAuth = () => {
    const context = useContext(AuthContext);
    if (!context) {
        throw new Error('useAuth must be used within AuthProvider');
    }
    return context;
};
```

#### 3. Login Component Example

```jsx
import React, { useState } from 'react';
import { useAuth } from './useAuth';

const LoginForm = () => {
    const [email, setEmail] = useState('');
    const [password, setPassword] = useState('');
    const [error, setError] = useState('');
    const [loading, setLoading] = useState(false);
    const { login } = useAuth();

    const handleSubmit = async (e) => {
        e.preventDefault();
        setError('');
        setLoading(true);

        try {
            await login(email, password);
            // Redirect will be handled by your routing logic
        } catch (err) {
            setError(err.message);
        } finally {
            setLoading(false);
        }
    };

    return (
        <form onSubmit={handleSubmit} className="auth-form">
            <h2>Login</h2>
            
            {error && <div className="error-message">{error}</div>}
            
            <div className="form-group">
                <label htmlFor="email">Email:</label>
                <input
                    id="email"
                    type="email"
                    value={email}
                    onChange={(e) => setEmail(e.target.value)}
                    required
                    disabled={loading}
                />
            </div>
            
            <div className="form-group">
                <label htmlFor="password">Password:</label>
                <input
                    id="password"
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    required
                    disabled={loading}
                />
            </div>
            
            <button type="submit" disabled={loading}>
                {loading ? 'Logging in...' : 'Login'}
            </button>
            
            <div className="form-footer">
                <p>Don't have an account? <a href="/register">Register here</a></p>
            </div>
        </form>
    );
};

export default LoginForm;
```

#### 4. Registration Component Example

```jsx
import React, { useState } from 'react';
import { useAuth } from './useAuth';

const RegisterForm = () => {
    const [formData, setFormData] = useState({
        email: '',
        password: '',
        confirmPassword: '',
        userType: 'investor'
    });
    const [error, setError] = useState('');
    const [loading, setLoading] = useState(false);
    const [success, setSuccess] = useState(false);
    const { register } = useAuth();

    const handleSubmit = async (e) => {
        e.preventDefault();
        setError('');
        setLoading(true);

        // Validate password confirmation
        if (formData.password !== formData.confirmPassword) {
            setError('Passwords do not match');
            setLoading(false);
            return;
        }

        try {
            await register(formData.email, formData.password, formData.userType);
            setSuccess(true);
        } catch (err) {
            setError(err.message);
        } finally {
            setLoading(false);
        }
    };

    if (success) {
        return (
            <div className="success-message">
                <h2>Registration Successful!</h2>
                <p>Your account has been created. You can now <a href="/login">login</a>.</p>
            </div>
        );
    }

    return (
        <form onSubmit={handleSubmit} className="auth-form">
            <h2>Register</h2>
            
            {error && <div className="error-message">{error}</div>}
            
            <div className="form-group">
                <label htmlFor="email">Email:</label>
                <input
                    id="email"
                    type="email"
                    value={formData.email}
                    onChange={(e) => setFormData({...formData, email: e.target.value})}
                    required
                    disabled={loading}
                />
            </div>
            
            <div className="form-group">
                <label htmlFor="userType">User Type:</label>
                <select
                    id="userType"
                    value={formData.userType}
                    onChange={(e) => setFormData({...formData, userType: e.target.value})}
                    disabled={loading}
                >
                    <option value="investor">Investor</option>
                    <option value="lender">Lender</option>
                </select>
            </div>
            
            <div className="form-group">
                <label htmlFor="password">Password:</label>
                <input
                    id="password"
                    type="password"
                    value={formData.password}
                    onChange={(e) => setFormData({...formData, password: e.target.value})}
                    required
                    disabled={loading}
                />
                <small>Password must be at least 8 characters with uppercase, lowercase, number, and special character.</small>
            </div>
            
            <div className="form-group">
                <label htmlFor="confirmPassword">Confirm Password:</label>
                <input
                    id="confirmPassword"
                    type="password"
                    value={formData.confirmPassword}
                    onChange={(e) => setFormData({...formData, confirmPassword: e.target.value})}
                    required
                    disabled={loading}
                />
            </div>
            
            <button type="submit" disabled={loading}>
                {loading ? 'Creating Account...' : 'Register'}
            </button>
            
            <div className="form-footer">
                <p>Already have an account? <a href="/login">Login here</a></p>
            </div>
        </form>
    );
};

export default RegisterForm;
```

#### 5. Protected Route Component

```jsx
import React from 'react';
import { useAuth } from './useAuth';

const ProtectedRoute = ({ children }) => {
    const { user, loading } = useAuth();

    if (loading) {
        return <div className="loading">Loading...</div>;
    }

    if (!user) {
        return (
            <div className="auth-required">
                <h2>Authentication Required</h2>
                <p>Please <a href="/login">login</a> to access this page.</p>
            </div>
        );
    }

    return children;
};

export default ProtectedRoute;
```

#### 6. API Client for Authenticated Requests

```javascript
import authService from './authService';

class ApiClient {
    constructor(baseUrl = '/api') {
        this.baseUrl = baseUrl;
    }

    async get(endpoint) {
        const response = await authService.authenticatedFetch(`${this.baseUrl}${endpoint}`);
        
        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || `HTTP ${response.status}`);
        }

        return await response.json();
    }

    async post(endpoint, data) {
        const response = await authService.authenticatedFetch(`${this.baseUrl}${endpoint}`, {
            method: 'POST',
            body: JSON.stringify(data),
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || `HTTP ${response.status}`);
        }

        return await response.json();
    }

    async put(endpoint, data) {
        const response = await authService.authenticatedFetch(`${this.baseUrl}${endpoint}`, {
            method: 'PUT',
            body: JSON.stringify(data),
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || `HTTP ${response.status}`);
        }

        return await response.json();
    }

    async delete(endpoint) {
        const response = await authService.authenticatedFetch(`${this.baseUrl}${endpoint}`, {
            method: 'DELETE',
        });

        if (!response.ok) {
            const error = await response.json();
            throw new Error(error.message || `HTTP ${response.status}`);
        }

        return response.status === 204 ? null : await response.json();
    }

    // Specific API methods
    async getUserDetails(userId) {
        return this.get(`/ccr/user/${userId}`);
    }

    async createLoan(loanData) {
        return this.post('/ccr/loans', loanData);
    }

    async getLoans() {
        return this.get('/ccr/loans');
    }

    async getBatches() {
        return this.get('/state/batches');
    }
}

const apiClient = new ApiClient();
export default apiClient;
```

## Environment Configuration

Create a `.env` file for configuration:

```env
# JWT Configuration
JWT_SECRET=your-super-secure-secret-key-here-at-least-32-characters-long
JWT_EXPIRY_HOURS=24

# Database
DATABASE_URL=postgresql://username:password@localhost/arda_credit

# Server
HOST=0.0.0.0
PORT=8080
RUST_LOG=info
```

## Security Best Practices

1. **Token Storage**: Consider using `httpOnly` cookies instead of localStorage for better security
2. **HTTPS**: Always use HTTPS in production
3. **Token Refresh**: Implement token refresh for better UX
4. **Error Handling**: Provide clear error messages without exposing sensitive information
5. **Validation**: Always validate input on both frontend and backend

## Common Integration Patterns

### With React Router
```jsx
import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { AuthProvider, useAuth } from './auth/useAuth';
import ProtectedRoute from './auth/ProtectedRoute';

function App() {
    return (
        <AuthProvider>
            <BrowserRouter>
                <Routes>
                    <Route path="/login" element={<LoginForm />} />
                    <Route path="/register" element={<RegisterForm />} />
                    <Route path="/dashboard" element={
                        <ProtectedRoute>
                            <Dashboard />
                        </ProtectedRoute>
                    } />
                    <Route path="/" element={<Navigate to="/dashboard" />} />
                </Routes>
            </BrowserRouter>
        </AuthProvider>
    );
}
```

### With Vue.js
```javascript
// auth.js
import { ref, computed } from 'vue';

const user = ref(null);
const token = ref(localStorage.getItem('arda_access_token'));

export const useAuth = () => {
    const isAuthenticated = computed(() => {
        if (!token.value) return false;
        try {
            const payload = JSON.parse(atob(token.value.split('.')[1]));
            return payload.exp > Math.floor(Date.now() / 1000);
        } catch {
            return false;
        }
    });

    const login = async (email, password) => {
        const response = await fetch('/api/auth/login', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ email, password }),
        });

        const data = await response.json();
        token.value = data.access_token;
        user.value = data.user;
        localStorage.setItem('arda_access_token', data.access_token);
        
        return data;
    };

    const logout = () => {
        token.value = null;
        user.value = null;
        localStorage.removeItem('arda_access_token');
    };

    return {
        user: user.value,
        isAuthenticated,
        login,
        logout,
    };
};
```

## Testing Authentication

### Unit Tests
```javascript
import authService from './authService';

describe('AuthService', () => {
    beforeEach(() => {
        localStorage.clear();
        fetch.resetMocks();
    });

    test('login stores token and user data', async () => {
        const mockResponse = {
            access_token: 'mock-jwt-token',
            user: { user_id: '123', email: 'test@example.com' },
        };

        fetch.mockResponseOnce(JSON.stringify(mockResponse));

        await authService.login('test@example.com', 'password');

        expect(localStorage.getItem('arda_access_token')).toBe('mock-jwt-token');
        expect(authService.getUser()).toEqual(mockResponse.user);
    });

    test('logout clears stored data', async () => {
        localStorage.setItem('arda_access_token', 'token');
        localStorage.setItem('arda_user_data', '{}');

        await authService.logout();

        expect(localStorage.getItem('arda_access_token')).toBeNull();
        expect(localStorage.getItem('arda_user_data')).toBeNull();
    });
});
```

This guide provides everything needed to integrate with the Arda Credit authentication system. The examples are production-ready and include proper error handling, security considerations, and common integration patterns.
