# CMake Language Specification — Reference voor Pest Grammar

Bronnen voor het implementeren van een Pest-gebaseerde CMake parser.

---

## 1. Officiële CMake Language Reference

**Bron:** [cmake-language(7)](https://cmake.org/cmake/help/latest/manual/cmake-language.7.html) — CMake 4.x documentatie.

### File structuur

```
file         ::= file_element*
file_element ::= command_invocation line_ending |
                  (bracket_comment | space)* line_ending
line_ending  ::= line_comment? newline
space        ::= <match '[ \t]+'>
newline      ::= <match '\n'>
```

### Command Invocations

```
command_invocation  ::= space* identifier space* '(' arguments ')'
identifier          ::= <match '[A-Za-z_][A-Za-z0-9_]*'>
arguments           ::= argument? separated_arguments*
separated_arguments ::= separation+ argument? |
                        separation* '(' arguments ')'
separation          ::= space | line_ending
```

- Command names zijn **case-insensitive**
- Geneste `(` `)` in argumenten moeten balanceren
- CMake < 3.0: identifier min. 2 karakters

### Argument types

```
argument ::= bracket_argument | quoted_argument | unquoted_argument
```

#### Bracket Argument (Lua-achtig)

```
bracket_argument ::= bracket_open bracket_content bracket_close
bracket_open     ::= '[' '='* '['
bracket_content  ::= <any text not containing bracket_close>
bracket_close    ::= ']' '='* ']'
```

- Lengte `=` moet overeenkomen: `[=[...]=]` vs `[[...]]`
- Geen escape of variabele-expansie in bracket content

#### Quoted Argument

```
quoted_argument      ::= '"' quoted_element* '"'
quoted_element       ::= <any except '\' or '"'> | escape_sequence | quoted_continuation
quoted_continuation  ::= '\' newline
```

- Escape sequences en `${var}` worden geëvalueerd
- `\` + newline = line continuation

#### Unquoted Argument

```
unquoted_argument ::= unquoted_element+ | unquoted_legacy
unquoted_element  ::= <any except whitespace, ()#"\> | escape_sequence
```

- Geen whitespace, `(`, `)`, `#`, `"`, `\` (behalve escaped)
- `;` deelt in lijsten (behalve `\;`)

### Escape Sequences

```
escape_sequence  ::= escape_identity | escape_encoded | escape_semicolon
escape_identity  ::= '\' <non-alphanumeric except ;>
escape_encoded   ::= '\t' | '\r' | '\n'
escape_semicolon ::= '\;'
```

### Variable References

- `${VAR}` — normale variabele
- `$ENV{VAR}` — environment variabele
- `$CACHE{VAR}` — cache entry
- Genest: `${outer_${inner}_variable}`

### Comments

- **Line comment:** `#` tot end of line
- **Bracket comment:** `#[[...]]` of `#[=[...]=]` (zelfde bracket-length regels)

### Control Structures

- `if()` / `elseif()` / `else()` / `endif()`
- `foreach()` / `endforeach()`
- `while()` / `endwhile()`
- `macro()` / `endmacro()`
- `function()` / `endfunction()`
- `block()` / `endblock()`

---

## 2. cmake-format Token Types (lex.py)

| Token | Beschrijving | Voorbeeld |
|-------|--------------|-----------|
| QUOTED_LITERAL | Quoted string | `"foo"` |
| BRACKET_ARGUMENT | Bracket-quoted | `[=[hello]=]` |
| WORD | Cmake entity name | `foo`, `add_executable` |
| DEREF | Variable dereference | `${foo}` |
| NUMBER | Numeric literal | `1234` |
| LEFT_PAREN / RIGHT_PAREN | `(` `)` | |
| NEWLINE / WHITESPACE | | |
| COMMENT / BRACKET_COMMENT | `# ...` of `#[[...]]` | |
| UNQUOTED_LITERAL | Andere non-whitespace | `hello.cc`, `--verbose` |

---

## 3. cmake-format Parse Tree Node Types

| Node | Children |
|------|----------|
| BODY | COMMENT, STATEMENT, WHITESPACE |
| STATEMENT | FUNNAME, ARGGROUP, LPAREN, RPAREN |
| FLOW_CONTROL | STATEMENT, BODY |
| ARGGROUP | PARGGROUP, KWARGGROUP, PARENGROUP, FLAGGROUP |
| PARGGROUP | ARGUMENT, COMMENT |
| FUNNAME | (token) |
| ARGUMENT | (token), COMMENT |

---

## 4. Bestaande Implementaties

### Rust: cmake-parser (crates.io)

- **Library:** `cmake-parser = "0.1"`
- **Methode:** nom (parser combinators), geen PEG
- **Scope:** CMake 3.26, 127 commands geïmplementeerd
- **API:** `parse_cmakelists()`, `Doc`, `Command` enum

### JavaScript: peg-cmake (twxs)

- **Methode:** PEG.js grammar
- **AST:** command_invocation, if, foreach, function, macro, while, bracket_argument, quoted_argument, etc.
- **Repo:** github.com/twxs/peg-cmake (grammar bestand mogelijk verplaatst)

---

## 5. ROS2/ament-specifieke Commands (voor babel42)

| Command | Rol | Extractie |
|---------|-----|-----------|
| `find_package(xxx REQUIRED)` | Dependencies | Package namen |
| `add_message_files(FILES a.msg b.msg)` | Interfaces | Msg bestanden |
| `add_service_files(FILES x.srv)` | Interfaces | Srv bestanden |
| `add_action_files(FILES y.action)` | Interfaces | Action bestanden |
| `ament_target_dependencies(target dep1 dep2)` | Link deps | Target + dependencies |
| `install(DIRECTORY launch/ DESTINATION ...)` | Install | Launch/config paths |

---

## 6. Pest-implementatie suggestie

1. **Lexical layer:** identifier, space, newline, line_comment, bracket_comment
2. **Arguments:** bracket_argument, quoted_argument, unquoted_argument (met escape_sequence)
3. **Command:** `identifier space* '(' arguments ')'` met nested `( arguments )`
4. **File:** sequence van (command_invocation | comment | whitespace)*
5. **Optioneel:** if/elseif/else/endif, foreach/endforeach voor flow control

Start simpel met command_invocation + arguments; flow control later toevoegen.
