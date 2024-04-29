use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use pyo3::exceptions::PyRuntimeError;
use numpy::IntoPyArray;

mod import_hash;
use import_hash::{DataType, Arrays};

fn data_type_to_py_object(py: Python, data: &DataType) -> PyResult<PyObject> {
    match data {
        DataType::String(val) => Ok(val.clone().into_py(py)),
        DataType::Int(val) => Ok((*val).into_py(py)),
        DataType::Float(val) => Ok((*val).into_py(py)),
        DataType::Bool(val) => Ok((*val).into_py(py)),
        DataType::HashMap(val) => {
            let dict = PyDict::new_bound(py);
            for (key, value) in val {
                dict.set_item(key, data_type_to_py_object(py, value)?)?;
            }
            Ok(dict.into())
        },
        DataType::VecInt(val) => {
            let list = PyList::new_bound(py, val);
            Ok(list.into())
        },
        DataType::VecChannel(val) => {
            let list = PyList::new_bound(py, &[]);
            for hashmap in val {
                let dict = PyDict::new_bound(py);
                for (key, value) in hashmap {
                    dict.set_item(key, data_type_to_py_object(py, value)?)?;
                }
                list.append(dict)?;
            }
            Ok(list.into())
        },
        DataType::Array(arrays) => {
            match arrays {
                Arrays::ArrayOne(array) => Ok(array.clone().into_pyarray_bound(py).into()),
                Arrays::ArrayTwo(array) => Ok(array.clone().into_pyarray_bound(py).into()),
                Arrays::ArrayTwoBool(array) => Ok(array.clone().into_pyarray_bound(py).into()),
            }
        },
        DataType::None => Ok(py.None()),
    }
}

#[pyfunction]
fn load_file_wrapper(py: Python, file_path: String) -> PyResult<(PyObject, bool)> {
    let result = import_hash::load_file(&file_path);
    match result {
        Ok((mut hash_map, flag)) => {
            let py_dict = PyDict::new_bound(py);
            for (key, value) in &mut hash_map {
                py_dict.set_item(key, data_type_to_py_object(py, value)?)?;
            }
            Ok((py_dict.into(), flag))
        },
        Err(e) => Err(PyRuntimeError::new_err(format!("{}", e))),
    }
}