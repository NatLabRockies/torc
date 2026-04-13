const BASE_DIR = dirname(dirname(Base.find_package("Torc")))
const SLEEP = joinpath(BASE_DIR, "..", "..", "python_client", "tests", "scripts", "sleep.py")

function create_workflow(api)
    return send_api_command(
        api,
        APIClient.create_workflow,
        APIClient.WorkflowModel(;
            user = "user",
            name = "test",
            description = "test workflow",
        ),
    )
end

function build_workflow(api, workflow)
    small = send_api_command(
        api,
        APIClient.create_resource_requirements,
        APIClient.ResourceRequirementsModel(;
            workflow_id = workflow.id,
            name = "small",
            num_cpus = 1,
            memory = "1g",
            runtime = "P0DT1H",
        ),
    )
    medium = send_api_command(
        api,
        APIClient.create_resource_requirements,
        APIClient.ResourceRequirementsModel(;
            workflow_id = workflow.id,
            name = "medium",
            num_cpus = 4,
            memory = "8g",
            runtime = "P0DT8H",
        ),
    )
    large = send_api_command(
        api,
        APIClient.create_resource_requirements,
        APIClient.ResourceRequirementsModel(;
            workflow_id = workflow.id,
            name = "large",
            num_cpus = 8,
            memory = "16g",
            runtime = "P0DT12H",
        ),
    )
    ud1 = send_api_command(
        api,
        APIClient.create_user_data,
        APIClient.UserDataModel(;
            workflow_id = workflow.id,
            name = "my_val1",
            is_ephemeral = false,
            data = Dict("key1" => "val1"),
        ),
    )
    ud2 = send_api_command(
        api,
        APIClient.create_user_data,
        APIClient.UserDataModel(;
            workflow_id = workflow.id,
            name = "my_val2",
            is_ephemeral = false,
            data = Dict("key2" => "val2"),
        ),
    )
    jobs = [
        APIClient.JobModel(;
            workflow_id = workflow.id,
            name = "sleep1",
            command = "python $SLEEP 1",
            resource_requirements_id = small.id,
        ),
        APIClient.JobModel(;
            workflow_id = workflow.id,
            name = "sleep2",
            command = "python $SLEEP 1",
            input_user_data_ids = [ud1.id],
            resource_requirements_id = medium.id,
        ),
        APIClient.JobModel(;
            workflow_id = workflow.id,
            name = "sleep2b",
            command = "python $SLEEP 1",
            input_user_data_ids = [ud1.id],
            resource_requirements_id = medium.id,
        ),
        APIClient.JobModel(;
            workflow_id = workflow.id,
            name = "sleep3",
            command = "python $SLEEP 1",
            input_user_data_ids = [ud2.id],
            resource_requirements_id = large.id,
        ),
    ]
    add_jobs(api, workflow.id, jobs)
end

function get_url()
    return get(
        ENV,
        "TORC_API_URL",
        "http://localhost:8080/torc-service/v1",
    )
end

@testset "Test workflow" begin
    url = get_url()
    api = make_api(url)
    workflow = create_workflow(api)
    output_dir = mktempdir()
    try
        build_workflow(api, workflow)
        result = run(`torc --url $url run $(workflow.id) --output-dir $output_dir`)
        @test result.exitcode == 0
        results, response = APIClient.list_results(api, workflow.id)
        @test response.status == 200
        for result in results.items
            @test result.return_code == 0
        end
    finally
        rm(output_dir; recursive = true)
        APIClient.delete_workflow(api, workflow.id)
    end
end

# Skip in CI due to path resolution issues with map_function_to_jobs
if haskey(ENV, "CI")
    @testset "Test mapped function workflow" begin
        @test_skip true
    end
else
    @testset "Test mapped function workflow" begin
        url = get_url()
        api = make_api(url)
        workflow = create_workflow(api)
        output_dir = mktempdir()
        params = [Dict("val" => i) for i in 1:5]
        project_path = BASE_DIR
        try
            jobs = map_function_to_jobs(
                api,
                workflow.id,
                joinpath(BASE_DIR, "test", "mapped_function.jl"),
                params;
                project_path = BASE_DIR,
                has_postprocess = true,
            )
            @test !isempty(jobs)
            result = run(`torc --url $url run $(workflow.id) --output-dir $output_dir`)
            @test result.exitcode == 0
            results, response = APIClient.list_results(api, workflow.id)
            @test response.status == 200
            for result in results.items
                @test result.return_code == 0
            end

            postprocess_job = jobs[end]
            result_ud, response = APIClient.list_user_data(
                api,
                workflow.id;
                producer_job_id = postprocess_job.id,
            )
            @test length(result_ud.items) == 1
            @test result_ud.items[1].data["total"] == 25
            @test "output_data_paths" in keys(result_ud.items[1].data)
        finally
            rm(output_dir; recursive = true)
            APIClient.delete_workflow(api, workflow.id)
        end
    end
end
