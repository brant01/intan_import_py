from setuptools import setup
from setuptools_rust import RustExtension

setup(
    name="my_module",
    version="0.1",
    rust_extensions=[RustExtension("my_module.my_module", binding="pyo3")],
    packages=["my_module"],
    zip_safe=False,
)