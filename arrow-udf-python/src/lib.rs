use anyhow::Result;
use arrow_array::{Array, ArrayRef, RecordBatch};
use arrow_schema::{DataType, Field, Schema};
use pyo3::types::{PyModule, PyTuple};
use pyo3::{PyObject, PyResult, Python};
use std::sync::Arc;

mod ffi;
mod pyarrow;

pub struct Runtime {
    function: PyObject,
}

impl Runtime {
    pub fn new(code: &str, function_name: &str) -> Result<Self> {
        pyo3::prepare_freethreaded_python();
        let function = Python::with_gil(|py| -> PyResult<PyObject> {
            Ok(PyModule::from_code(py, code, "", "")?
                .getattr(function_name)?
                .into())
        })?;
        Ok(Self { function })
    }

    pub fn call(&self, input: &RecordBatch) -> Result<RecordBatch> {
        // convert each row to python objects and call the function
        let array = Python::with_gil(|py| -> Result<ArrayRef> {
            let mut results = Vec::with_capacity(input.num_rows());
            let mut row = vec![];
            for i in 0..input.num_rows() {
                for column in input.columns() {
                    let pyobj = pyarrow::get_pyobject(py, column, i);
                    row.push(pyobj);
                }
                let args = PyTuple::new(py, row.drain(..));
                let result = self.function.call1(py, args)?;
                results.push(result);
            }
            let result = pyarrow::build_array(&DataType::Int32, py, &results)?;
            Ok(result)
        })?;
        let schema = Schema::new(vec![Field::new("result", array.data_type().clone(), true)]);
        Ok(RecordBatch::try_new(Arc::new(schema), vec![array])?)
    }
}
