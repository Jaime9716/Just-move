# Biblioteca compartida que expone contracts/engine_api.h.
# engine.cpp -> agente C · sources.cpp -> agente A. No editar el archivo del otro.
add_library(jd_engine SHARED engine.cpp sources.cpp)
target_compile_definitions(jd_engine PRIVATE JD_BUILDING_ENGINE)
set_target_properties(jd_engine PROPERTIES CXX_VISIBILITY_PRESET hidden)
target_link_libraries(jd_engine PRIVATE jd_common)
foreach(lib jd_input jd_session jd_signal jd_features jd_choreo jd_judge)
  if(TARGET ${lib})
    target_link_libraries(jd_engine PRIVATE ${lib})
  endif()
endforeach()
