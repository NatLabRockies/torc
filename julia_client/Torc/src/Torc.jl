module Torc

include("api/APIClient.jl")
import OpenAPI
import .APIClient

function make_api(api_url::AbstractString)
    """Instantiate an OpenAPI object from an API URL."""
    return APIClient.DefaultApi(OpenAPI.Clients.Client(api_url))
end

"""
Send a request through the client and throw an exception if it fails.
"""
function send_api_command(api::APIClient.DefaultApi, func, args...; kwargs...)
    data, response = func(api, args...; kwargs...)

    if response.status != 200
        error("Failed to send_api_command: $(response)")
    end

    return data
end

"""
Add an iterable of jobs to the workflow.
"""
function add_jobs(
    api::APIClient.DefaultApi,
    workflow_id::Int64,
    jobs,
    max_transfer_size = 100_000,
)
    added_jobs = []
    batch = []
    for job in jobs
        push!(batch, job)
        if length(batch) > max_transfer_size
            res = send_api_command(
                api,
                APIClient.create_jobs,
                APIClient.JobsModel(; jobs = batch),
            )
            added_jobs = vcat(added_jobs, res.jobs)
            empty!(batch)
        end
    end

    if length(batch) > 0
        res = send_api_command(
            api,
            APIClient.create_jobs,
            APIClient.JobsModel(; jobs = batch),
        )
        added_jobs = vcat(added_jobs, res.jobs)
    end

    return added_jobs
end

"""
Add one job to the workflow for each set of parameters.

# Arguments
- `api::APIClient.DefaultApi`: API instance
- `workflow_id::Int64`: Workflow ID
- `file_path::AbstractString`: Path to script that Torc will execute.
- `params::Vector`: Torc will create one job for each set of parameters.
- `project_path = nothing`: If set, will pass this path to julia --project=X when running the
   script.
- `has_postprocess = false`: Set to true if the script defines a postprocess function.
- `resource_requirements_id = nothing`: If set, Torc will use this resource requirements ID.
- `scheduler_id = nothing`: If set, Torc will use this scheduler ID.
- `start_index = 1`: Torc will use this index for job names.
- `name_prefix = ""`: Torc will use this prefix for job names.
- `job_names = String[]`: Use these names for jobs. Mutually exclusive with "name_prefix."
- `depends_on_job_ids::Union{Nothing, Vector{Int64}} = nothing`: Set these job IDs as blocking
   the jobs created by this function.
- `cancel_on_blocking_job_failure::Bool = true`: Cancel each job if a blocking job fails.
"""
function map_function_to_jobs(
    api::APIClient.DefaultApi,
    workflow_id::Int64,
    file_path::AbstractString,
    params::Vector;
    project_path = nothing,
    has_postprocess = false,
    resource_requirements_id = nothing,
    scheduler_id = nothing,
    start_index = 1,
    name_prefix = "",
    job_names::Vector{String} = String[],
    depends_on_job_ids::Union{Nothing, Vector{Int64}} = nothing,
    cancel_on_blocking_job_failure = true,
)
    !isfile(file_path) && error("$file_path does not exist")
    if !isempty(job_names) && length(job_names) != length(params)
        error("If job_names is provided, it must be the same length as params.")
    end
    jobs = Vector{APIClient.JobModel}()
    output_data_ids = Vector{Int64}()
    ppath = isnothing(project_path) ? "" : "--project=$(project_path)"
    url = api.client.root
    command = "julia $ppath $(file_path) $(url)"

    for (i, job_params) in enumerate(params)
        if !isempty(job_names)
            job_name = job_names[i]
        else
            job_name = "$(name_prefix)$(start_index + i)"
        end
        input_ud = send_api_command(
            api,
            APIClient.create_user_data,
            APIClient.UserDataModel(;
                workflow_id = workflow_id,
                name = "input_$(job_name)",
                data = Dict{String, Any}("params" => job_params)),
        )
        output_ud = send_api_command(
            api,
            APIClient.create_user_data,
            APIClient.UserDataModel(;
                workflow_id = workflow_id,
                name = "output_$(job_name)",
                data = Dict{String, Any}(),
            ),
        )
        @assert !isnothing(input_ud.id)
        @assert !isnothing(output_ud.id)
        push!(output_data_ids, output_ud.id)
        job = APIClient.JobModel(;
            workflow_id = workflow_id,
            name = job_name,
            command = command * " run",
            input_user_data_ids = [input_ud.id],
            output_user_data_ids = [output_ud.id],
            resource_requirements_id = resource_requirements_id,
            scheduler_id = scheduler_id,
            depends_on_job_ids = depends_on_job_ids,
            cancel_on_blocking_job_failure = cancel_on_blocking_job_failure,
        )
        push!(jobs, job)
    end

    if has_postprocess
        output_ud = send_api_command(
            api,
            APIClient.create_user_data,
            APIClient.UserDataModel(;
                workflow_id = workflow_id,
                name = "postprocess_result",
                data = Dict{String, Any}(),
            ),
        )
        @assert !isnothing(output_ud.id)
        push!(jobs,
            APIClient.JobModel(;
                workflow_id = workflow_id,
                name = "postprocess",
                command = command * " postprocess",
                input_user_data_ids = output_data_ids,
                output_user_data_ids = [output_ud.id],
                resource_requirements_id = resource_requirements_id,
                scheduler_id = scheduler_id,
            ),
        )
    end

    return add_jobs(api, workflow_id, jobs)
end

"""
Return the current user.
"""
function get_user()
    return Sys.iswindows() ? get(ENV, "USERNAME", nothing) : get(ENV, "USER", nothing)
end

include("map_function.jl")

export make_api
export send_api_command
export add_jobs
export get_user
export map_function_to_jobs
export process_mapped_function_cli_args

end # module Torc
