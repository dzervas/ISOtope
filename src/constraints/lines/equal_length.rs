use std::{cell::RefCell, rc::Rc};

use crate::{constraints::Constraint, primitives::line::Line};

// This is a sketch constraint that makes the end point of an arc coincident with a point.
#[derive(Debug)]
pub struct EqualLength {
    line1: Rc<RefCell<Line>>,
    line2: Rc<RefCell<Line>>,
}

impl EqualLength {
    pub fn new(line1: Rc<RefCell<Line>>, line2: Rc<RefCell<Line>>) -> Self {
        Self { line1, line2 }
    }

    pub fn line1(&self) -> Rc<RefCell<Line>> {
        self.line1.clone()
    }

    pub fn set_line1(&mut self, line1: Rc<RefCell<Line>>) {
        self.line1 = line1;
    }

    pub fn line2(&self) -> Rc<RefCell<Line>> {
        self.line2.clone()
    }

    pub fn set_line2(&mut self, line2: Rc<RefCell<Line>>) {
        self.line2 = line2;
    }
}

impl Constraint for EqualLength {
    fn references(&self) -> Vec<Rc<RefCell<dyn crate::primitives::Parametric>>> {
        vec![self.line1.clone(), self.line2.clone()]
    }

    fn loss_value(&self) -> f64 {
        let start1 = self.line1.borrow().start().borrow().data();
        let end1 = self.line1.borrow().end().borrow().data();
        let start2 = self.line2.borrow().start().borrow().data();
        let end2 = self.line2.borrow().end().borrow().data();

        let d1 = end1 - start1;
        let d2 = end2 - start2;

        let difference = d1.norm() - d2.norm();
        0.5 * difference * difference
    }

    fn update_gradient(&mut self) {
        let start1 = self.line1.borrow().start().borrow().data();
        let end1 = self.line1.borrow().end().borrow().data();
        let start2 = self.line2.borrow().start().borrow().data();
        let end2 = self.line2.borrow().end().borrow().data();

        let d1 = end1 - start1;
        let d2 = end2 - start2;

        let difference = d1.norm() - d2.norm();
        let _loss = 0.5 * difference * difference;

        let grad_from_difference = difference;
        let grad_difference_from_d1 = d1.transpose() / d1.norm();
        let grad_difference_from_d2 = -d2.transpose() / d2.norm();

        let grad_start1 = self.line1.borrow().start_gradient();
        let grad_end1 = self.line1.borrow().end_gradient();
        let grad_start2 = self.line2.borrow().start_gradient();
        let grad_end2 = self.line2.borrow().end_gradient();

        self.line1.borrow_mut().add_to_gradient(
            (grad_from_difference * grad_difference_from_d1 * (grad_end1 - grad_start1)).as_view(),
        );

        self.line2.borrow_mut().add_to_gradient(
            (grad_from_difference * grad_difference_from_d2 * (grad_end2 - grad_start2)).as_view(),
        );
    }
}

// Run some tests
#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use crate::{
        constraints::{lines::equal_length::EqualLength, Constraint},
        primitives::{line::Line, point2::Point2},
        sketch::Sketch,
        solvers::gradient_based_solver::GradientBasedSolver,
    };

    #[test]
    fn test_equal_length() {
        let sketch = Rc::new(RefCell::new(Sketch::new()));

        let line1_start = Rc::new(RefCell::new(Point2::new(3.0, 4.0)));
        let line1_end = Rc::new(RefCell::new(Point2::new(5.0, 6.0)));
        let line1 = Rc::new(RefCell::new(Line::new(
            line1_start.clone(),
            line1_end.clone(),
        )));
        sketch
            .borrow_mut()
            .add_primitive(line1_start.clone())
            .unwrap();
        sketch
            .borrow_mut()
            .add_primitive(line1_end.clone())
            .unwrap();
        sketch.borrow_mut().add_primitive(line1.clone()).unwrap();

        let line2_start = Rc::new(RefCell::new(Point2::new(0.0, 4.0)));
        let line2_end = Rc::new(RefCell::new(Point2::new(10.0, 6.0)));
        let line2 = Rc::new(RefCell::new(Line::new(
            line2_start.clone(),
            line2_end.clone(),
        )));

        sketch
            .borrow_mut()
            .add_primitive(line2_start.clone())
            .unwrap();
        sketch
            .borrow_mut()
            .add_primitive(line2_end.clone())
            .unwrap();
        sketch.borrow_mut().add_primitive(line2.clone()).unwrap();

        let constr1 = Rc::new(RefCell::new(EqualLength::new(line1.clone(), line2.clone())));
        sketch.borrow_mut().add_constraint(constr1.clone()).unwrap();

        sketch
            .borrow_mut()
            .check_gradients(1e-6, constr1.clone(), 1e-6);
        let solver = GradientBasedSolver::new(sketch.clone());
        solver.solve();

        println!(
            "line1 len: {:?}",
            (line1_end.as_ref().borrow().data() - line1_start.as_ref().borrow().data()).norm()
        );
        println!(
            "line2 len: {:?}",
            (line2_end.as_ref().borrow().data() - line2_start.as_ref().borrow().data()).norm()
        );

        println!("line1: {:?}", line1.as_ref().borrow());
        println!("line2: {:?}", line2.as_ref().borrow());

        assert!(constr1.as_ref().borrow().loss_value() < 0.001);
    }
}
