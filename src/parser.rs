use crate::instruction::*;
use crate::isa;
use crate::lexer::*;
use crate::program_state::DataWord;
use crate::program_state::IRegister;
use std::collections::HashMap;
use std::fmt;
use std::iter::Peekable;
use std::vec::IntoIter;

#[derive(Eq, PartialEq, Debug)]
struct ParseErrorData {
    msg: String,
    contents: String,
}

#[derive(Eq, PartialEq, Debug)]
pub struct ParseError {
    location: Location,
    data: ParseErrorData,
}

impl ParseError {
    pub fn new(location: Location, msg: String, contents: String) -> Self {
        ParseError {
            location,
            data: ParseErrorData { msg, contents },
        }
    }

    fn bad_head(location: Location, contents: String) -> Self {
        ParseError::new(
            location,
            "Expected label, section, or instruction".to_string(),
            contents,
        )
    }

    fn bad_arg(location: Location, contents: String) -> Self {
        ParseError::new(
            location,
            "Expected register name or immediate".to_string(),
            contents,
        )
    }

    fn bad_reg(location: Location, reg_name: String) -> Self {
        ParseError::new(location, "Expected register name".to_string(), reg_name)
    }

    fn not_enough_args(location: Location, contents: String) -> Self {
        ParseError::new(location, "Not enough arguments".to_string(), contents)
    }

    fn unexpected(location: Location, contents: String) -> Self {
        ParseError::new(location, "Unexpected token".to_string(), contents)
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ParseError at {}: {}\t{}",
            self.location, self.data.msg, self.data.contents
        )
    }
}

#[derive(Clone)]
enum ParseType {
    R(&'static dyn Fn(IRegister, IRegister, IRegister) -> ConcreteInst),
    Arith(&'static dyn Fn(IRegister, IRegister, DataWord) -> ConcreteInst),
    Env,
    MemL,
    MemS,
    B,
    Jal,
    Jalr,
    U,
}

struct ParserData {
    inst_expansion_table: HashMap<String, ParseType>,
    reg_expansion_table: HashMap<String, IRegister>,
}

pub struct RiscVParser {
    parser_data: ParserData,
    lines: LineTokenStream,
}

type TokenIter<'a> = Peekable<IntoIter<Token>>;

impl RiscVParser {
    pub fn from_tokens(lines: LineTokenStream) -> Self {
        use isa::*;
        use ParseType::*;
        let inst_expansion_table = [
            ("add", R(&Add::new)),
            ("addi", Arith(&Addi::new)),
            ("and", R(&And::new)),
            ("andi", Arith(&Andi::new)),
            ("auipc", U),
            ("beq", B),
            ("bge", B),
            ("bgeu", B),
            ("blt", B),
            ("bltu", B),
            ("bne", B),
            ("ebreak", Env),
            ("ecall", Env),
            ("jal", ParseType::Jal),
            ("jalr", ParseType::Jalr),
            ("lb", MemL),
            ("lbu", MemL),
            ("lh", MemL),
            ("lhu", MemL),
            ("lui", U),
            ("lw", MemL),
            // ("or", R),
            // ("ori", Arith),
            ("sb", MemS),
            ("sh", MemS),
            // ("sll", R),
            // ("slli", Arith),
            // ("slt", R),
            // ("slti", Arith),
            // ("sltiu", Arith),
            // ("sltu", R),
            // ("sra", R),
            // ("srai", Arith),
            // ("srl", R),
            // ("srli", Arith),
            // ("sub", R),
            ("sw", MemS),
            // ("xor", R),
            // ("xori", Arith),
        ]
        .iter()
        .cloned()
        .map(|(s, t)| (s.to_owned(), t))
        .collect();
        let mut reg_expansion_table: HashMap<String, IRegister> = IRegister::REG_ARRAY
            .iter()
            .map(|r| (r.to_string(), *r))
            .collect();
        for i in 0..32 {
            reg_expansion_table.insert(format!("x{}", i), IRegister::from(i));
        }
        // don't forget FP
        reg_expansion_table.insert("fp".to_string(), IRegister::FP);
        RiscVParser {
            parser_data: ParserData {
                inst_expansion_table,
                reg_expansion_table,
            },
            lines,
        }
    }

    pub fn parse(self) -> (Vec<ConcreteInst>, Vec<ParseError>) {
        let mut insts = Vec::<ConcreteInst>::new();
        let mut errs = Vec::<ParseError>::new();
        for line in self.lines {
            match LineParser::new(&self.parser_data, line).parse() {
                Ok(new_insts) => insts.extend(new_insts),
                Err(new_err) => errs.push(new_err),
            }
        }
        (insts, errs)
    }
}

/// Returns the first error found in lst, or none if there are no such errors.
fn find_first_err<T, E>(lst: Vec<Result<T, E>>) -> Option<E> {
    lst.into_iter().find_map(|r| match r {
        Err(e) => Some(e),
        _ => None,
    })
}

struct LineParser<'a> {
    data: &'a ParserData,
    iter: TokenIter<'a>,
}

impl LineParser<'_> {
    /// Attempts to consume exactly N arguments from the iterator, possibly comma-separated.
    /// The first and last tokens cannot be commas. If repeated commas appear anywhere,
    /// an error is returned.
    /// The only tokens that may appear during this consumption are commas, names, and immediates.
    fn consume_commasep_args(
        &mut self,
        head_loc: Location,
        head_name: String,
        n: usize,
    ) -> Result<Vec<Token>, ParseError> {
        use TokenType::*;
        if n == 0 {
            if self.iter.peek().is_some() {
                Err(ParseError::unexpected(
                    head_loc,
                    format!(
                        "{:?} (too many arguments for instruction {})",
                        self.iter.next().unwrap(),
                        head_name
                    ),
                ))
            } else {
                Ok(Vec::new())
            }
        } else {
            match self.iter.next() {
                Some(tok) => match tok.data {
                    Name(..) | Immediate(..) => {
                        // Allow single comma, excpet when trailing
                        if n > 1 {
                            if let Some(tok2) = self.iter.peek() {
                                if let Comma = tok2.data {
                                    self.iter.next();
                                }
                            }
                        }
                        self.consume_commasep_args(head_loc, head_name, n - 1)
                            .and_then(|mut args| {
                                args.insert(0, tok);
                                Ok(args)
                            })
                    }
                    _ => Err(ParseError::bad_arg(tok.location, format!("{:?}", tok.data))),
                },
                None => Err(ParseError::not_enough_args(
                    head_loc,
                    format!("for instruction {}", head_name),
                )),
            }
        }
    }

    fn try_parse_reg(&self, token: &Token) -> Result<IRegister, ParseError> {
        match &token.data {
            TokenType::Name(name) => self
                .data
                .reg_expansion_table
                .get(name)
                .cloned()
                .ok_or_else(|| ParseError::bad_reg(token.location, name.to_string())),
            _ => Err(ParseError::unexpected(
                token.location,
                format!("Expected register name, found {:?}", token.data),
            )),
        }
    }

    fn try_parse_imm(&self, token: &Token) -> Result<DataWord, ParseError> {
        match &token.data {
            TokenType::Immediate(val, ..) => Ok(DataWord::from(*val)),
            _ => Err(ParseError::unexpected(
                token.location,
                format!("Expected immediate, found {:?}", token.data),
            )),
        }
    }

    fn try_expand_inst(
        &mut self,
        head_loc: Location,
        name: String,
    ) -> Result<Vec<ConcreteInst>, ParseError> {
        use ParseType::*;
        if let Some(parse_type) = self.data.inst_expansion_table.get(name.as_str()) {
            match parse_type {
                R(inst_new) => {
                    // R-types are always "inst rd, rs1, rs2" with one or no commas in between
                    let args = self.consume_commasep_args(head_loc, name, 3)?;
                    debug_assert!(args.len() == 3);
                    let regs: Vec<Result<IRegister, ParseError>> =
                        args.iter().map(|arg| self.try_parse_reg(arg)).collect();
                    match regs.as_slice() {
                        [Ok(rd), Ok(rs1), Ok(rs2)] => Ok(vec![inst_new(*rd, *rs1, *rs2)]),
                        _ => Err(find_first_err(regs).unwrap()),
                    }
                }
                Arith(inst_new) => {
                    let args = self.consume_commasep_args(head_loc, name, 3)?;
                    debug_assert!(args.len() == 3);
                    let rd = self.try_parse_reg(&args[0])?;
                    let rs1 = self.try_parse_reg(&args[1])?;
                    let imm = self.try_parse_imm(&args[2])?;
                    Ok(vec![inst_new(rd, rs1, imm)])
                }
                // // Env => ,
                // Mem => ,
                // B => ,
                // Jal => ,
                // Jalr => ,
                // U => ,
                _ => Err(ParseError::unexpected(
                    head_loc,
                    "unimplemented".to_string(),
                )),
            }
        } else {
            Err(ParseError::new(
                head_loc,
                "No instruction found with name".to_string(),
                name,
            ))
        }
    }

    fn new(data: &ParserData, tokens: TokenStream) -> LineParser {
        LineParser {
            data,
            iter: tokens.into_iter().peekable(),
        }
    }

    fn parse(mut self) -> Result<Vec<ConcreteInst>, ParseError> {
        // needed for lifetime reasons i guess
        if let Some(head_tok) = self.iter.next() {
            use TokenType::*;
            match head_tok.data {
                Name(name) => self.try_expand_inst(head_tok.location, name),
                LabelDef(_label_name) => Ok(Vec::new()), // TODO
                SectionDef(_section_name) => Ok(Vec::new()), // TODO
                Comment(..) => Ok(Vec::new()),           // deliberate no-op
                Comma => Err(ParseError::bad_head(head_tok.location, ",".to_string())),
                Immediate(n, style) => {
                    Err(ParseError::bad_head(head_tok.location, style.format(n)))
                }
                LParen => Err(ParseError::bad_head(head_tok.location, "(".to_string())),
                RParen => Err(ParseError::bad_head(head_tok.location, ")".to_string())),
            }
        } else {
            Ok(Vec::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isa::*;
    use crate::lexer;
    use crate::program_state::IRegister::*;

    #[test]
    fn test_bad_commas() {
        let bad_insts = vec![
            "add x5, sp, fp,",
            "add ,x1, x2, x3",
            ",add x1 x2 x3",
            "add x1,,x2, x3",
        ];
        for inst in bad_insts {
            let (toks, lex_err) = lexer::Lexer::from_string(inst.to_string()).lex();
            assert!(lex_err.is_empty());
            let parser = RiscVParser::from_tokens(toks);
            let (_, parse_err) = parser.parse();
            assert!(!parse_err.is_empty());
        }
    }

    #[test]
    fn test_r_type_parse() {
        let (toks, lex_err) = lexer::Lexer::from_string("add x5, sp, fp".to_string()).lex();
        assert!(lex_err.is_empty());
        let parser = RiscVParser::from_tokens(toks);
        let (insts, parse_err) = parser.parse();
        assert!(parse_err.is_empty());
        assert_eq!(insts.len(), 1);
        assert_eq!(insts[0], Add::new(IRegister::from(5), SP, FP));
    }

    #[test]
    fn test_i_arith_parse() {
        // lack of commas is deliberate
        let (toks, lex_err) = lexer::Lexer::from_string("addi sp sp -4".to_string()).lex();
        assert!(lex_err.is_empty());
        let parser = RiscVParser::from_tokens(toks);
        let (insts, parse_err) = parser.parse();
        assert!(parse_err.is_empty());
        assert_eq!(insts.len(), 1);
        assert_eq!(insts[0], Addi::new(SP, SP, DataWord::from(-4)));
    }
}
