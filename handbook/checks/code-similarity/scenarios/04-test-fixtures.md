# Scenario 4: Test fixtures and test classes (Java)

Test code that spans multiple files with different roles: a fixture class
providing test data, and a test class consuming it. Both live under `tests/`.

```java
// package/tests/fixtures/MyFixture.java
class MyFixture {
    Something givenSomething() {
        return new Something(1);
    }

    Something givenSomethingElse() {
        return new Something(2);
    }
}

// package/tests/SomethingTest.java
@RunWith(JUnit4.class)
class SomethingTest {
    @Test
    void given_something_when_do_then_oh_no() {
        something = MyFixture.givenSomething();
        result = something.doIt();
        assertFalse(result);
    }

    @Test
    void given_something_else_when_do_then_oh_yes() {
        something = MyFixture.givenSomethingElse();
        result = something.doIt();
        assertTrue(result);
    }
}
```

Two clone pairs: the fixture methods (factory pattern) and the test methods
(AAA pattern). Both are expected similarity.

**Assumption:** plain classes (no contract) are transparent containers. Our
model looks through them. A class only becomes a node when it implements a
contract. May need revisiting.

## Parse

```
Source("package/tests/fixtures/MyFixture.java")
  └── TestRegion
        ├── Token("$ID")    ← givenSomething tokens
        ├── ...
        ├── Token("$ID")    ← givenSomethingElse tokens
        ├── ...

Source("package/tests/SomethingTest.java")
  └── TestRegion
        ├── Token("$ID")    ← test method 1 tokens
        ├── ...
        ├── Token("$ID")    ← test method 2 tokens
        ├── ...
```

Both files are under `tests/`, so both are wrapped in TestRegion. The fixture
class has no `@Test` annotations, but it's test infrastructure. Its location
makes it test context.

## Detect

Flatten, suffix array. Finds clone pairs within each file.

## Evaluate

Walk up from all occurrences. Both pairs: all tokens inside TestRegion →
test thresholds → **warn**.

## Open question

Should fixture similarity and test similarity be treated the same? Fixtures
are shared infrastructure, not throwaway AAA boilerplate. That's a rule design
question, not a data structure question. The data structure supports
distinguishing them if needed (different container types).
