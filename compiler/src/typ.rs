pub mod check {
    use crate::hir::ir::{
        universe::{Expression, Level, Universe},
        Declaration, Inductive, Term, HIR,
    };
    use environment::{global, local};

    mod environment {
        pub mod global {
            use crate::hir::ir::{Inductive, Term};

            pub struct Environment {
                declarations: Vec<Declaration>,
                // TODO: `universe_graph: uGraph.t`
            }

            impl Environment {
                pub fn new() -> Environment {
                    Environment {
                        declarations: Vec::new(),
                    }
                }

                pub fn lookup_inductive(&self, name: &str) -> Inductive {
                    for declaration in &self.declarations {
                        if let Declaration::Inductive(inductive_name, inductive) = declaration {
                            if inductive_name == name {
                                return inductive.clone();
                            }
                        }
                    }
                    panic!()
                }

                pub fn push_inductive(&mut self, inductive: Inductive) {
                    self.declarations
                        .push(Declaration::Inductive(inductive.name.clone(), inductive))
                }
            }

            enum Declaration {
                Constant(String, ConstantBody),
                Inductive(String, Inductive),
            }

            struct ConstantBody {
                typ: Term,
                body: Option<Term>,
                // TODO: `universes: universe_context`
            }
        }

        pub mod local {
            use crate::hir::ir::{Name, Term};

            pub struct Environment {
                pub declarations: Vec<Declaration>,
            }

            impl Environment {
                pub fn new() -> Environment {
                    Environment {
                        declarations: Vec::new(),
                    }
                }

                pub fn push_declaration(&mut self, name: Name, typ: Term) {
                    self.declarations.push(Declaration {
                        name,
                        body: None,
                        typ,
                    })
                }

                pub fn pop_declaration(&mut self) {
                    self.declarations.pop();
                }
            }

            #[derive(Debug)]
            pub struct Declaration {
                name: Name,
                body: Option<Term>,
                pub typ: Term,
            }
        }
    }

    pub fn type_check_hir(hir: &HIR) {
        let mut global = global::Environment::new();
        let mut local = local::Environment::new();
        for declaration in hir.declarations.iter() {
            match declaration {
                Declaration::Constant(term) => {
                    type_check_term(&global, &mut local, term);
                    // TODO: add this to the global environment
                }
                Declaration::Inductive(inductive) => type_check_inductive(&mut global, &inductive),
            }
        }
    }

    pub fn type_check_fresh_term(term: &Term) -> Term {
        type_check_term(
            &global::Environment::new(),
            &mut local::Environment::new(),
            term,
        )
    }

    // assert when type checking fails
    // TODO: return error messages using ariadne.
    fn type_check_term(
        global: &global::Environment,
        local: &mut local::Environment,
        term: &Term,
    ) -> Term {
        match term {
            Term::DeBruijnIndex(debruijn_index) => {
                // pass only if the `debruijn_index` is a local declaration
                local.declarations.get(*debruijn_index).unwrap().typ.clone()
            }
            Term::Sort(universe) => {
                assert_eq!(universe.length(), 1);
                let universe_expression = universe.first();
                assert_eq!(universe_expression.1, false);
                match universe_expression.level() {
                    Level::Prop | Level::Set => {
                        Term::Sort(Universe::build_one(Expression::type_1()))
                    } // return Type 1
                    _ => Term::Sort(Universe::build_one(universe_expression.successor())),
                }
            }
            Term::DependentProduct {
                parameter_name,
                parameter_type,
                return_type,
            } => {
                let parameter_type_type = type_check_term(global, local, parameter_type);
                let parameter_type_universe = match parameter_type_type {
                    Term::Sort(ref universe) => universe,
                    _ => panic!("{:#?}", parameter_type),
                };

                local.push_declaration(parameter_name.clone(), parameter_type_type.clone());
                let return_type_type = type_check_term(global, local, return_type);
                let return_type_universe = match return_type_type {
                    Term::Sort(ref universe) => universe,
                    _ => panic!("{:#?}", return_type),
                };
                local.pop_declaration();

                Term::Sort(Universe::sort_of_product(
                    parameter_type_universe,
                    return_type_universe,
                ))
            }
            Term::Lambda {
                parameter_name,
                parameter_type,
                body,
            } => {
                type_check_term(global, local, parameter_type);

                local.push_declaration(parameter_name.clone(), (**parameter_type).clone());
                let body_type = type_check_term(global, local, body);
                local.pop_declaration();

                Term::DependentProduct {
                    parameter_name: parameter_name.clone(),
                    parameter_type: parameter_type.clone(),
                    return_type: Box::new(body_type),
                }
            }
            Term::Inductive(identifier, _) => {
                let _inductive = global.lookup_inductive(identifier);

                // TODO: handle universes
                term.clone()
            }
            _ => todo!("{:#?}", term),
        }
    }

    pub fn type_check_fresh_inductive(inductive: &Inductive) {
        type_check_inductive(&mut global::Environment::new(), inductive);
    }

    fn well_formed_arity(arity: &Term) {
        match arity {
            Term::DependentProduct { return_type, .. } => well_formed_arity(&*return_type),
            Term::Sort(_) => (),
            _ => panic!("arity is not well formed"),
        }
    }

    // assert when type checking fails
    // TODO: return error messages using ariadne.
    fn type_check_inductive(global: &mut global::Environment, inductive: &Inductive) {
        well_formed_arity(&inductive.arity);
        type_check_fresh_term(&inductive.arity);

        global.push_inductive(inductive.clone());
        let mut constructor_sorts = inductive.constructors.iter().map(|constructor| {
            type_check_term(global, &mut local::Environment::new(), &constructor.typ)
        });
        if let Some(first_constructor_sort) = constructor_sorts.next() {
            assert!(constructor_sorts
                .all(|constructor_sort| constructor_sort == first_constructor_sort))
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::hir::ir::{universe::UniverseInstance, Constructor, Name};

        #[test]
        fn identity_function() {
            let parameter_type = Box::new(Term::Sort(Universe::build_one(Expression::set())));
            assert_eq!(
                type_check_fresh_term(&Term::Lambda {
                    parameter_name: Name::Named("a".to_string()),
                    parameter_type: parameter_type.clone(),
                    body: Box::new(Term::DeBruijnIndex(0)),
                }),
                Term::DependentProduct {
                    parameter_name: Name::Named("a".to_string()),
                    parameter_type: parameter_type.clone(),
                    return_type: parameter_type,
                }
            )
        }

        #[test]
        #[should_panic]
        fn identity_function_malformed() {
            type_check_fresh_term(&Term::Lambda {
                parameter_name: Name::Named("a".to_string()),
                parameter_type: Box::new(Term::Sort(Universe::build_one(Expression::set()))),
                body: Box::new(Term::DeBruijnIndex(1)),
            });
        }

        fn inductive_nat() -> Inductive {
            let natural = "Natural".to_string();

            Inductive {
                name: natural.clone(),
                parameters: Vec::new(),
                arity: Term::Sort(Universe::build_one(Expression::set())),
                constructors: vec![
                    Constructor {
                        name: "Zero".to_string(),
                        typ: Term::Inductive(natural.clone(), UniverseInstance::empty()),
                    },
                    Constructor {
                        name: "Successor".to_string(),
                        typ: Term::DependentProduct {
                            parameter_name: Name::Anonymous,
                            parameter_type: Box::new(Term::Inductive(
                                natural.clone(),
                                UniverseInstance::empty(),
                            )),
                            return_type: Box::new(Term::Inductive(
                                natural.clone(),
                                UniverseInstance::empty(),
                            )),
                        },
                    },
                ],
            }
        }

        #[test]
        fn nat_type() {
            type_check_fresh_inductive(&inductive_nat())
        }

        #[test]
        fn nat_add() {
            // func rec add(a b : Nat) -> Nat {
            //     match a -> Nat {
            //       Nat.O => b
            //       Nat.S(x : Nat) => Nat.S(add(x, b))
            //     }
            // }

            let nat = inductive_nat();

            let nat_term = Box::new(Term::Inductive(nat.name.clone(), UniverseInstance::empty()));
            let a = Name::Named("a".to_string());
            let b = Name::Named("b".to_string());

            let recursive_add = Term::Fixpoint {
                fixpoint_name: Name::Named("add".to_string()),
                fixpoint_type: Box::new(Term::DependentProduct {
                    parameter_name: a.clone(),
                    parameter_type: nat_term.clone(),
                    return_type: Box::new(Term::DependentProduct {
                        parameter_name: b.clone(),
                        parameter_type: nat_term.clone(),
                        return_type: nat_term.clone(),
                    }),
                }),
                body: Box::new(Term::Lambda {
                    parameter_name: a.clone(),
                    parameter_type: nat_term.clone(),
                    body: Box::new(Term::Lambda {
                        parameter_name: b.clone(),
                        parameter_type: nat_term.clone(),
                        body: Box::new(Term::Match {
                            inductive_name: nat.name.clone(),
                            parameter_count: 0,
                            type_info: Box::new(Term::Lambda {
                                parameter_name: a.clone(),
                                parameter_type: nat_term.clone(),
                                body: nat_term.clone(),
                            }),
                            discriminee: Box::new(Term::DeBruijnIndex(1)),
                            branches: vec![
                                (0, Term::DeBruijnIndex(0)),
                                (
                                    1,
                                    Term::Lambda {
                                        parameter_name: Name::Named("x".to_string()),
                                        parameter_type: nat_term.clone(),
                                        body: Box::new(Term::Application {
                                            function: Box::new(Term::Constructor(
                                                nat.name.clone(),
                                                1,
                                                UniverseInstance::empty(),
                                            )),
                                            arguments: vec![Term::Application {
                                                function: Box::new(Term::DeBruijnIndex(3)),
                                                arguments: vec![
                                                    Term::DeBruijnIndex(0),
                                                    Term::DeBruijnIndex(1),
                                                ],
                                            }],
                                        }),
                                    },
                                ),
                            ],
                        }),
                    }),
                }),
                recursive_parameter_index: 0,
            };

            type_check_hir(&HIR {
                declarations: vec![Declaration::Inductive(nat), Declaration::Constant(recursive_add)],
            });
        }
    }
}
