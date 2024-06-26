use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use nalgebra::{DMatrix, DVector};

use crate::error::ISOTopeError;

use super::{constraints::Constraint, primitives::Parametric};

#[derive(Default)]
pub struct Sketch {
    primitives: VecDeque<Rc<RefCell<dyn Parametric>>>,
    constraints: VecDeque<Rc<RefCell<dyn Constraint>>>,
}

impl Sketch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_primitive(
        &mut self,
        primitive: Rc<RefCell<dyn Parametric>>,
    ) -> Result<(), ISOTopeError> {
        // Make sure all referenced primitives are added to the sketch before the primitive
        for reference in primitive.borrow().references() {
            if !self.primitives.iter().any(|p| Rc::ptr_eq(p, &reference)) {
                return Err(ISOTopeError::MissingSketchReferences);
            }
        }
        // Check that the primitive is not already in the sketch
        if self.primitives.iter().any(|p| Rc::ptr_eq(p, &primitive)) {
            return Err(ISOTopeError::PrimitiveAlreadyInSketch);
        }
        // Add the primitive to the sketch
        self.primitives.push_back(primitive);

        Ok(())
    }

    pub fn add_constraint(
        &mut self,
        constraint: Rc<RefCell<dyn Constraint>>,
    ) -> Result<(), ISOTopeError> {
        // Make sure all referenced primitives are added to the sketch before the constraint
        for reference in constraint.borrow().references() {
            if !self.primitives.iter().any(|p| Rc::ptr_eq(p, &reference)) {
                return Err(ISOTopeError::MissingSketchReferences);
            }
        }
        // Make sure the constraint is not already in the sketch
        if self.constraints.iter().any(|c| Rc::ptr_eq(c, &constraint)) {
            return Err(ISOTopeError::ConstraintAlreadyInSketch);
        }

        self.constraints.push_back(constraint);

        Ok(())
    }

    pub fn primitives(&self) -> VecDeque<Rc<RefCell<dyn Parametric>>> {
        self.primitives.clone()
    }

    pub fn get_n_dofs(&self) -> usize {
        let mut n_dofs = 0;
        for primitive in self.primitives.iter() {
            n_dofs += primitive.borrow().get_data().len();
        }
        n_dofs
    }

    pub fn get_data(&self) -> DVector<f64> {
        let mut data = DVector::zeros(self.get_n_dofs());
        let mut i = 0;
        for primitive in self.primitives.iter() {
            let primitive_data = primitive.borrow().get_data();
            data.rows_mut(i, primitive_data.len())
                .copy_from(&primitive_data);
            i += primitive_data.len();
        }
        data
    }

    pub fn get_loss(&mut self) -> f64 {
        let mut loss = 0.0;
        for constraint in self.constraints.iter_mut() {
            loss += constraint.borrow().loss_value();
        }
        loss
    }

    pub fn get_gradient(&mut self) -> DVector<f64> {
        for primitive in self.primitives.iter_mut() {
            primitive.borrow_mut().zero_gradient();
        }

        for constraint in self.constraints.iter_mut() {
            constraint.borrow_mut().update_gradient();
        }

        let mut gradient = DVector::zeros(self.get_n_dofs());
        let mut i = 0;
        for primitive in self.primitives.iter() {
            let primitive_gradient = primitive.borrow().get_gradient();
            gradient
                .rows_mut(i, primitive_gradient.len())
                .copy_from(&primitive_gradient);
            i += primitive_gradient.len();
        }
        gradient
    }

    pub fn get_loss_per_constraint(&self) -> DVector<f64> {
        let mut loss_per_constraint = DVector::zeros(self.constraints.len());
        for (i, constraint) in self.constraints.iter().enumerate() {
            loss_per_constraint[i] = constraint.borrow().loss_value();
        }
        loss_per_constraint
    }

    pub fn get_jacobian(&self) -> DMatrix<f64> {
        let mut jacobian = DMatrix::zeros(self.constraints.len(), self.get_n_dofs());
        for (i, constraint) in self.constraints.iter().enumerate() {
            // Zero the gradients of all primitives
            for primitive in self.primitives.iter() {
                primitive.borrow_mut().zero_gradient();
            }
            // Update the gradient of the constraint
            constraint.borrow_mut().update_gradient();
            // Copy the gradient of the constraint to the jacobian
            let mut j = 0;
            for primitive in self.primitives.iter() {
                let primitive_gradient = primitive.borrow().get_gradient();
                jacobian
                    .row_mut(i)
                    .columns_mut(j, primitive_gradient.len())
                    .copy_from(&primitive_gradient.transpose());
                j += primitive_gradient.len();
            }
        }
        jacobian
    }

    pub fn set_data(&mut self, data: DVector<f64>) {
        assert!(data.len() == self.get_n_dofs());
        let mut i = 0;
        for primitive in self.primitives.iter_mut() {
            let n = primitive.borrow().get_data().len();
            primitive.borrow_mut().set_data(data.rows(i, n).as_view());
            i += n;
        }
    }

    // This function is used in test cases to check the gradients of the primitives
    pub fn check_gradients(
        &mut self,
        epsilon: f64,
        constraint: Rc<RefCell<dyn Constraint>>,
        check_epsilon: f64,
    ) {
        // Update all gradients
        self.get_gradient();

        // Compare to numerical gradients
        let constraint_loss = constraint.borrow().loss_value();
        for primitive in self.primitives.iter_mut() {
            let original_value = primitive.borrow().get_data();
            let analytical_gradient = primitive.borrow().get_gradient();
            let mut numerical_gradient = DVector::zeros(original_value.len());
            let n = primitive.borrow().get_data().len();
            assert!(n == analytical_gradient.len());
            for i in 0..n {
                let mut new_value = original_value.clone();
                new_value[i] += epsilon;
                primitive.borrow_mut().set_data(new_value.clone().as_view());
                let new_loss = constraint.borrow().loss_value();
                primitive
                    .borrow_mut()
                    .set_data(original_value.clone().as_view());
                numerical_gradient[i] = (new_loss - constraint_loss) / epsilon;
            }

            println!("Analytical gradient: {:?}", analytical_gradient);
            println!("Numerical gradient: {:?}", numerical_gradient);

            let error = (numerical_gradient - analytical_gradient).norm();
            println!("Error: {}", error);
            assert!(error < check_epsilon);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        constraints::coincident::arc_end_point_coincident::ArcEndPointCoincident,
        examples::test_rectangle_rotated::RotatedRectangleDemo,
        primitives::{arc::Arc, point2::Point2},
    };

    use super::*;

    #[test]
    fn test_references_have_to_be_added_beforehand() {
        assert!(std::panic::catch_unwind(|| {
            let mut sketch = Sketch::new();

            let point = Rc::new(RefCell::new(Point2::new(0.0, 0.0)));
            let arc = Rc::new(RefCell::new(Arc::new(point, 1.0, true, 0.0, 1.0)));

            sketch.add_primitive(arc.clone()).unwrap();
        })
        .is_err());
    }

    #[test]
    fn test_primitive_cannot_be_added_twice() {
        assert!(std::panic::catch_unwind(|| {
            let mut sketch = Sketch::new();

            let point = Rc::new(RefCell::new(Point2::new(0.0, 0.0)));
            sketch.add_primitive(point.clone()).unwrap();
            sketch.add_primitive(point.clone()).unwrap();
        })
        .is_err());
    }

    #[test]
    fn test_constraint_references_have_to_be_added_beforehand() {
        assert!(std::panic::catch_unwind(|| {
            let mut sketch = Sketch::new();

            let point = Rc::new(RefCell::new(Point2::new(0.0, 0.0)));
            let arc = Rc::new(RefCell::new(Arc::new(point.clone(), 1.0, true, 0.0, 1.0)));

            sketch.add_primitive(point.clone()).unwrap();

            let constraint = Rc::new(RefCell::new(ArcEndPointCoincident::new(arc, point)));
            sketch.add_constraint(constraint).unwrap();
        })
        .is_err());
    }

    #[test]
    fn test_constraint_cannot_be_added_twice() {
        assert!(std::panic::catch_unwind(|| {
            let mut sketch = Sketch::new();

            let point = Rc::new(RefCell::new(Point2::new(0.0, 0.0)));
            let arc = Rc::new(RefCell::new(Arc::new(point.clone(), 1.0, true, 0.0, 1.0)));

            sketch.add_primitive(point.clone()).unwrap();
            sketch.add_primitive(arc.clone()).unwrap();

            let constraint = Rc::new(RefCell::new(ArcEndPointCoincident::new(
                arc.clone(),
                point.clone(),
            )));
            sketch.add_constraint(constraint.clone()).unwrap();
            sketch.add_constraint(constraint.clone()).unwrap();
        })
        .is_err());
    }

    #[test]
    fn test_data_and_grad_functions() {
        let rect = RotatedRectangleDemo::new();
        let mut sketch = rect.sketch.borrow_mut();
        sketch.get_data();
        sketch.get_loss();
        sketch.get_gradient();
        sketch.get_loss_per_constraint();
        sketch.get_jacobian();
    }
}
