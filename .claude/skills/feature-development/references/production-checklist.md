# Production Readiness Checklist

A feature isn't done when the code works. It's done when you can observe it working *and* know if users find it valuable.

## Contents
- Error Handling
- Technical Observability
- Product Analytics
- Resilience
- Performance
- Accessibility
- Data & State
- Security
- Applying This Checklist

---

## Error Handling

- [ ] **Graceful degradation**: Feature fails safely, doesn't crash the system
- [ ] **Meaningful messages**: Errors help users understand what happened and what to do
- [ ] **No swallowed exceptions**: All errors logged or handled explicitly
- [ ] **Boundary validation**: Untrusted input rejected early with clear feedback
- [ ] **Retry logic**: Transient failures retried where appropriate (with backoff)

Consider:
- Behavior when a dependency is unavailable
- User experience when something goes wrong
- Whether support can diagnose issues from error messages alone

---

## Technical Observability

Ensure you can tell if the feature is working correctly in production.

- [ ] **Logging**: Key events logged (feature invoked, completed, failed)
- [ ] **Log levels appropriate**: Debug for details, Info for events, Error for failures
- [ ] **No sensitive data in logs**: PII, secrets, tokens excluded
- [ ] **Correlation IDs**: Requests traceable across services
- [ ] **Metrics**: Key measurements exposed (latency, throughput, error rate)
- [ ] **Alerts**: Know when something breaks before users report it
- [ ] **Health checks**: Feature health included in system health

Consider:
- How you'd know if this feature stopped working at 3am
- Whether you can trace a single user's request through the system
- Which dashboards need updating

---

## Product Analytics

Ensure you can tell if the feature is *valuable* to users. Technical observability shows the feature works. Product analytics shows it matters.

- [ ] **Usage tracking**: Feature invocations measured (who, when, how often)
- [ ] **Adoption metrics**: Users trying the feature, users returning
- [ ] **Success metrics**: Users completing the intended flow
- [ ] **Drop-off points**: Where users abandon the feature
- [ ] **Performance perception**: Latency impact on user behavior

Consider:
- How you'll know if this feature is successful
- User behavior that indicates value
- What would trigger a decision to iterate, pivot, or remove
- Whether product managers can self-serve usage answers

### Instrumentation Guidance

Think in terms of **events** that tell a story:

```
feature_started     → user initiated the feature
feature_step_N      → user progressed through flow
feature_completed   → user achieved the goal
feature_abandoned   → user left before completing
feature_error       → something went wrong
```

Include context that enables segmentation:
- User cohort (new vs returning, plan tier, etc.)
- Entry point (how did they get here?)
- Variant (if A/B testing)

Avoid:
- Tracking everything "just in case" (adds noise, costs money)
- Vanity metrics that don't drive decisions
- PII in analytics events

---

## Resilience

Ensure the feature handles real-world conditions.

- [ ] **Timeouts**: External calls have appropriate timeouts
- [ ] **Circuit breakers**: Repeated failures stop cascading
- [ ] **Rate limiting**: Protected against abuse/overload
- [ ] **Backpressure**: Handles load spikes gracefully
- [ ] **Idempotency**: Safe to retry operations
- [ ] **Fallbacks**: Degraded experience better than no experience

Consider:
- Behavior at 10x expected traffic
- Behavior when downstream service is slow
- Protection against endpoint spamming

---

## Performance

Ensure the feature is fast enough for users.

- [ ] **Latency budget**: Response time acceptable for the use case
- [ ] **Database queries**: N+1 queries avoided, indexes in place
- [ ] **Caching**: Appropriate caching strategy (if applicable)
- [ ] **Payload size**: Response size reasonable, pagination for large datasets
- [ ] **Async where appropriate**: Long operations don't block user flow

Consider:
- Expected vs acceptable latency
- Behavior at 10x, 100x current load
- Obvious bottlenecks

When to address:
- Don't optimize prematurely — make it work first
- Do address obvious inefficiencies (N+1 queries, missing indexes)
- Do set latency budgets for user-facing operations
- Profile before optimizing; measure after

---

## Accessibility

For user-facing features: ensure all users can access it.

- [ ] **Keyboard navigation**: All functionality reachable without mouse
- [ ] **Screen readers**: Semantic HTML, ARIA labels where needed
- [ ] **Color contrast**: Text readable, not color-only indicators
- [ ] **Focus management**: Logical focus order, visible focus states
- [ ] **Error identification**: Errors announced and associated with fields

Consider:
- Keyboard-only user completing the flow
- Screen reader conveying the same information
- Color-only indicators that need alternatives

Mark N/A if purely backend or internal tooling with no UI.

---

## Data & State

- [ ] **Data migrations**: Schema changes handled safely
- [ ] **Backwards compatibility**: Old clients still work
- [ ] **Rollback plan**: Can revert without data loss
- [ ] **Data retention**: Compliant with policies

---

## Security

Walk through [OWASP Top 10](https://owasp.org/www-project-top-ten/) for the feature:

- [ ] **Injection**: Inputs sanitized, parameterized queries used
- [ ] **Broken authentication**: Session handling secure
- [ ] **Sensitive data exposure**: Data encrypted at rest/transit, no PII leaks
- [ ] **Broken access control**: Authorization enforced at every layer
- [ ] **Security misconfiguration**: Defaults hardened, errors don't leak internals
- [ ] **XSS**: Output encoded, CSP headers in place
- [ ] **Insecure deserialization**: Untrusted data not deserialized blindly
- [ ] **Vulnerable components**: Dependencies scanned, no known CVEs
- [ ] **Insufficient logging**: Security events auditable
- [ ] **SSRF**: Server-side requests validated

Also:
- [ ] **Secrets management**: No hardcoded credentials, secrets rotatable
- [ ] **Least privilege**: Feature requests only permissions it needs

---

## Applying This Checklist

Not every item applies to every feature. For each item:
- **Address it**, or
- **Mark N/A with reason** (e.g., "N/A: no external calls, timeout not relevant")

The goal isn't checkbox compliance — it's conscious consideration of production realities.
